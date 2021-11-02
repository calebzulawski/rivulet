use super::{SplittableSource, SplittableSourceMut};
use futures::task::AtomicWaker;
use pin_project::pin_project;
use std::{
    convert::TryInto,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

#[pin_project]
pub struct Source<T>
where
    T: SplittableSource,
{
    #[pin]
    source: T,
    waker: Arc<AtomicWaker>,
    head: u64,
    len: usize,
}

impl<T> Source<T>
where
    T: SplittableSource,
{
    pub(crate) fn new(waker: Arc<AtomicWaker>, source: T) -> Self {
        Self {
            source,
            waker,
            head: 0,
            len: 0,
        }
    }
}

impl<T> crate::View for Source<T>
where
    T: SplittableSource + Unpin,
{
    type Item = T::Item;
    type Error = T::Error;

    fn view(&self) -> &[Self::Item] {
        // we have unique ownership of the source, so this doesn't overlap with any other views
        unsafe { self.source.view(self.head, self.len) }
    }

    fn poll_grant(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::Error>> {
        let pinned = self.project();
        match pinned.source.as_ref().poll_available(
            cx,
            |waker| pinned.waker.register(waker),
            *pinned.head,
            count,
        ) {
            Poll::Ready(Ok(len)) => {
                *pinned.len = len;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn release(&mut self, count: usize) {
        self.len -= count;
        let count: u64 = count.try_into().unwrap();
        self.head += count;
        self.source.set_head(self.head);
    }
}

impl<T> crate::ViewMut for Source<T>
where
    T: SplittableSourceMut + Unpin,
{
    fn view_mut(&mut self) -> &mut [Self::Item] {
        // we have unique ownership of the source, so this doesn't overlap with any other views
        unsafe { self.source.view_mut(self.head, self.len) }
    }
}

impl<T> crate::Source for Source<T> where T: SplittableSource + Unpin {}
