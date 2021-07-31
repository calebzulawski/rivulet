//! Lazy-initialized streams.

use crate::{Sink, Source, View, ViewMut};
use core::{
    pin::Pin,
    sync::atomic::AtomicBool,
    task::{Context, Poll},
};
use pin_project::pin_project;

/// A lazy-initialized view.
///
/// The view is only initialized when polled for a grant.
#[pin_project]
#[derive(Copy, Clone, Debug, Hash)]
pub struct Lazy<V, F> {
    view: Option<V>,
    init: Option<F>,
}

impl<V, F> Lazy<V, F> {
    /// Create a new lazy view.
    pub fn new(init: F) -> Self {
        Self {
            view: None,
            init: Some(init),
        }
    }

    /// Return the inner type, if it has been initialized.
    pub fn into_inner(self) -> Option<V> {
        self.view
    }
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std"))))]
impl<V> Lazy<V, Box<dyn FnOnce() -> V>> {
    /// Create a new lazy view with a boxed initialization function.
    pub fn new_boxed(init: impl FnOnce() -> V + 'static) -> Self {
        Self::new(Box::new(init))
    }
}

impl<V, F> View for Lazy<V, F>
where
    V: View + Unpin,
    F: FnOnce() -> V,
{
    type Item = V::Item;
    type Error = V::Error;

    fn view(&self) -> &[Self::Item] {
        if let Some(view) = self.view.as_ref() {
            view.view()
        } else {
            &[]
        }
    }

    fn poll_grant(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::Error>> {
        if count > 0 {
            let this = self.project();
            if this.view.is_none() {
                this.view.get_or_insert(this.init.take().unwrap()());
            }
            Pin::new(this.view.as_mut().unwrap()).poll_grant(cx, count)
        } else {
            Poll::Ready(Ok(()))
        }
    }

    fn release(&mut self, count: usize) {
        if count > 0 {
            self.view
                .as_mut()
                .expect("attempted to release greater than grant")
                .release(count)
        }
    }
}

impl<V, F> ViewMut for Lazy<V, F>
where
    V: ViewMut + Unpin,
    F: FnOnce() -> V,
{
    fn view_mut(&mut self) -> &mut [Self::Item] {
        if let Some(view) = self.view.as_mut() {
            view.view_mut()
        } else {
            &mut []
        }
    }
}

impl<V, F> Sink for Lazy<V, F>
where
    V: Sink + Unpin,
    F: FnOnce() -> V,
{
}

impl<V, F> Source for Lazy<V, F>
where
    V: Source + Unpin,
    F: FnOnce() -> V,
{
}

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std"))))]
mod channel {
    use super::*;
    use core::marker::PhantomData;
    use std::sync::{atomic::Ordering, Arc, Mutex};

    struct LazyChannelImpl<Sink, Source, F> {
        ready: AtomicBool,
        source: Mutex<Option<Source>>,
        init: Mutex<Option<F>>,
        _sink: PhantomData<Sink>,
    }

    impl<Sink, Source, F> LazyChannelImpl<Sink, Source, F>
    where
        F: FnOnce() -> (Sink, Source),
    {
        fn new(f: F) -> Self {
            Self {
                ready: AtomicBool::new(false),
                source: Mutex::new(None),
                init: Mutex::new(Some(f)),
                _sink: PhantomData,
            }
        }

        fn take_sink(&self) -> Sink {
            let init = self.init.lock().unwrap().take().unwrap();
            let (sink, source) = init();
            self.source.lock().unwrap().replace(source);
            self.ready.store(true, Ordering::Release);
            sink
        }

        fn try_take_source(&self) -> Option<Source> {
            if self.ready.load(Ordering::Acquire) {
                self.source.lock().unwrap().take()
            } else {
                None
            }
        }
    }

    /// A sink created by [`lazy_channel`].
    #[pin_project]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "std"))))]
    pub struct LazyChannelSink<Sink, Source, F> {
        view: Option<Sink>,
        shared: Arc<LazyChannelImpl<Sink, Source, F>>,
    }

    impl<Sink, Source, F> View for LazyChannelSink<Sink, Source, F>
    where
        Sink: crate::View + Unpin,
        F: FnOnce() -> (Sink, Source),
    {
        type Item = Sink::Item;
        type Error = Sink::Error;

        fn view(&self) -> &[Self::Item] {
            if let Some(view) = self.view.as_ref() {
                view.view()
            } else {
                &[]
            }
        }

        fn poll_grant(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Self::Error>> {
            if count > 0 {
                let this = self.project();
                if this.view.is_none() {
                    this.view.get_or_insert(this.shared.take_sink());
                }
                Pin::new(this.view.as_mut().unwrap()).poll_grant(cx, count)
            } else {
                Poll::Ready(Ok(()))
            }
        }

        fn release(&mut self, count: usize) {
            if count > 0 {
                self.view
                    .as_mut()
                    .expect("attempted to release greater than grant")
                    .release(count)
            }
        }
    }

    impl<Sink, Source, F> ViewMut for LazyChannelSink<Sink, Source, F>
    where
        Sink: ViewMut + Unpin,
        F: FnOnce() -> (Sink, Source),
    {
        fn view_mut(&mut self) -> &mut [Self::Item] {
            if let Some(view) = self.view.as_mut() {
                view.view_mut()
            } else {
                &mut []
            }
        }
    }

    impl<Sink, Source, F> crate::Sink for LazyChannelSink<Sink, Source, F>
    where
        Sink: crate::Sink + Unpin,
        F: FnOnce() -> (Sink, Source),
    {
    }

    impl<Sink, Source, F> crate::Source for LazyChannelSink<Sink, Source, F>
    where
        Sink: crate::Source + Unpin,
        F: FnOnce() -> (Sink, Source),
    {
    }

    /// A source created by [`lazy_channel`].
    #[pin_project]
    #[cfg_attr(docsrs, doc(cfg(all(feature = "std"))))]
    pub struct LazyChannelSource<Sink, Source, F> {
        view: Option<Source>,
        shared: Arc<LazyChannelImpl<Sink, Source, F>>,
    }

    impl<Sink, Source, F> View for LazyChannelSource<Sink, Source, F>
    where
        Source: View + Unpin,
        F: FnOnce() -> (Sink, Source),
    {
        type Item = Source::Item;
        type Error = Source::Error;

        fn view(&self) -> &[Self::Item] {
            if let Some(view) = self.view.as_ref() {
                view.view()
            } else {
                &[]
            }
        }

        fn poll_grant(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Self::Error>> {
            if count > 0 {
                let this = self.project();
                if this.view.is_none() {
                    if let Some(source) = this.shared.try_take_source() {
                        this.view.get_or_insert(source);
                    }
                }
                Pin::new(this.view.as_mut().unwrap()).poll_grant(cx, count)
            } else {
                Poll::Ready(Ok(()))
            }
        }

        fn release(&mut self, count: usize) {
            if count > 0 {
                self.view
                    .as_mut()
                    .expect("attempted to release greater than grant")
                    .release(count)
            }
        }
    }

    impl<Sink, Source, F> ViewMut for LazyChannelSource<Sink, Source, F>
    where
        Source: ViewMut + Unpin,
        F: FnOnce() -> (Sink, Source),
    {
        fn view_mut(&mut self) -> &mut [Self::Item] {
            if let Some(view) = self.view.as_mut() {
                view.view_mut()
            } else {
                &mut []
            }
        }
    }

    impl<Sink, Source, F> crate::Sink for LazyChannelSource<Sink, Source, F>
    where
        Source: crate::Sink + Unpin,
        F: FnOnce() -> (Sink, Source),
    {
    }

    impl<Sink, Source, F> crate::Source for LazyChannelSource<Sink, Source, F>
    where
        Source: crate::Source + Unpin,
        F: FnOnce() -> (Sink, Source),
    {
    }

    /// Create a lazy-initialized channel.
    ///
    /// The channel is only initialized when first writing to the sink.
    #[cfg_attr(docsrs, doc(cfg(all(feature = "std"))))]
    pub fn lazy_channel<Sink, Source, F>(
        f: F,
    ) -> (
        LazyChannelSink<Sink, Source, F>,
        LazyChannelSource<Sink, Source, F>,
    )
    where
        F: FnOnce() -> (Sink, Source) + 'static,
        Sink: 'static,
        Source: 'static,
    {
        let shared = Arc::new(LazyChannelImpl::new(f));

        (
            LazyChannelSink {
                view: None,
                shared: shared.clone(),
            },
            LazyChannelSource { view: None, shared },
        )
    }
}

#[cfg(feature = "std")]
pub use channel::*;
