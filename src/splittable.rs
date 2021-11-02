use futures::task::AtomicWaker;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

mod source;
pub use source::*;

mod split;
pub use split::*;

pub trait Splittable {
    type Source: SplittableSource;

    fn into_impl(self, signal: impl Fn()) -> Self::Source;

    fn into_source(self) -> Source<Self::Source>
    where
        Self: Sized,
    {
        let signal = Arc::new(AtomicWaker::new());
        Source::new(signal.clone(), self.into_impl(move || signal.wake()))
    }

    fn split(self) -> Split<Self::Source>
    where
        Self: Sized,
    {
        let signal = Arc::new(SplitSignal::new());
        Split::new(signal.clone(), self.into_impl(move || signal.wake()))
    }
}

pub unsafe trait SplittableSource {
    type Item;
    type Error: core::fmt::Debug;

    fn set_head(&mut self, index: u64);

    fn compare_set_head(&self, index: u64);

    fn poll_available(
        self: Pin<&Self>,
        cx: &mut Context,
        register_wakeup: impl FnOnce(&Waker),
        index: u64,
        len: usize,
    ) -> Poll<Result<usize, Self::Error>>;

    unsafe fn view(&self, index: u64, len: usize) -> &[Self::Item];
}

pub unsafe trait SplittableSourceMut: SplittableSource {
    unsafe fn view_mut(&self, index: u64, len: usize) -> &mut [Self::Item];
}
