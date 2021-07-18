use core::{
    pin::Pin,
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
impl<V> Lazy<V, Box<dyn FnOnce() -> V>> {
    /// Create a new lazy view with a boxed initialization function.
    pub fn new_boxed(init: impl FnOnce() -> V + 'static) -> Self {
        Self::new(Box::new(init))
    }
}

impl<V, F> crate::View for Lazy<V, F>
where
    V: crate::View + Unpin,
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

impl<V, F> crate::ViewMut for Lazy<V, F>
where
    V: crate::ViewMut + Unpin,
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

impl<V, F> crate::Sink for Lazy<V, F>
where
    V: crate::Sink + Unpin,
    F: FnOnce() -> V,
{
}

impl<V, F> crate::Source for Lazy<V, F>
where
    V: crate::Source + Unpin,
    F: FnOnce() -> V,
{
}
