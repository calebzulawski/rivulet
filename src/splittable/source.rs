use super::{Splittable, SplittableMut};
use futures::task::AtomicWaker;
use pin_project::pin_project;
use std::{
    convert::TryInto,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

/// A source returned by [`Splittable::into_source`](`super::Splittable::into_source`).
#[pin_project]
pub struct Source<T>
where
    T: Splittable,
{
    #[pin]
    splittable: T,
    waker: Arc<AtomicWaker>,
    head: u64,
    len: usize,
}

impl<T> Source<T>
where
    T: Splittable,
{
    pub(crate) fn new(mut splittable: T) -> Self {
        let waker = Arc::new(AtomicWaker::new());
        // Safety: we have unique ownership of this
        unsafe {
            let waker = waker.clone();
            splittable.set_reader_waker(move || waker.wake());
        }
        Self {
            splittable,
            waker,
            head: 0,
            len: 0,
        }
    }
}

impl<T> crate::View for Source<T>
where
    T: Splittable,
{
    type Item = T::Item;
    type Error = T::Error;

    fn view(&self) -> &[Self::Item] {
        // we have unique ownership of the source, so this doesn't overlap with any other views
        unsafe { self.splittable.view(self.head, self.len) }
    }

    fn poll_grant(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::Error>> {
        let pinned = self.project();
        match pinned.splittable.as_ref().poll_available(
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
        // Safety: we never read earlier than this head value
        unsafe {
            self.splittable.set_head(self.head);
        }
    }
}

impl<T> crate::ViewMut for Source<T>
where
    T: SplittableMut,
{
    fn view_mut(&mut self) -> &mut [Self::Item] {
        // we have unique ownership of the source, so this doesn't overlap with any other views
        unsafe { self.splittable.view_mut(self.head, self.len) }
    }
}

impl<T> crate::Source for Source<T> where T: Splittable {}
