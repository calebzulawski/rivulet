use futures::task::AtomicWaker;
use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll, Waker},
};

mod source;
pub use source::*;

mod cloneable_source;
pub use cloneable_source::*;

/// A source that can be split for use with multiple readers.
pub trait Splittable {
    /// The source implementation type.
    type Source: SplittableSource;

    /// Create a `SplittableSource`.
    ///
    /// `signal` is called by the implementation to signal more data is available, and tasks should
    /// wake.
    ///
    /// You probably don't need to call this function directly.
    fn into_impl(self, signal: impl Fn()) -> Self::Source;

    /// Create a source for a single reader.
    fn into_source(self) -> Source<Self::Source>
    where
        Self: Sized,
    {
        let signal = Arc::new(AtomicWaker::new());
        Source::new(signal.clone(), self.into_impl(move || signal.wake()))
    }

    /// Create a source that implements `Clone`.
    fn into_cloneable_source(self) -> Cloneable<Self::Source>
    where
        Self: Sized,
    {
        let signal = Arc::new(CloneableSignal::new());
        Cloneable::new(signal.clone(), self.into_impl(move || signal.wake()))
    }
}

/// Similar to [`crate::Source`], but can be used with multiple readers.
pub unsafe trait SplittableSource {
    /// The streamed type.
    type Item;

    /// The error produced by [`poll_available`](`Self::poll_available`).
    type Error: core::fmt::Debug;

    /// Set the earliest position retained in the stream.
    ///
    /// # Panics
    /// May panic if the provided an index earlier than the current head.
    fn set_head(&mut self, index: u64);

    /// Set the earliest position retained in the stream.
    ///
    /// This function, unlike [`set_head`](`Self::set_head`), is synchronized between threads.
    /// If the provided head is less than the current head, the current head remains unchanged.
    fn compare_set_head(&self, index: u64);

    /// Suspends the current task until `len` samples starting at `index` are available, returning
    /// the available length.
    ///
    /// If the stream is closed, the returned available length may be less than `len`.
    fn poll_available(
        self: Pin<&Self>,
        cx: &mut Context,
        register_wakeup: impl FnOnce(&Waker),
        index: u64,
        len: usize,
    ) -> Poll<Result<usize, Self::Error>>;

    /// Obtain a view into the stream.
    ///
    /// # Safety
    /// The parameters must be within an available window as returned by
    /// [`poll_available`](`Self::poll_available`).
    unsafe fn view(&self, index: u64, len: usize) -> &[Self::Item];
}

/// A mutable source that can be used with multiple readers.
pub unsafe trait SplittableSourceMut: SplittableSource {
    /// Obtain a mutable view into the stream.
    ///
    /// # Safety
    /// * The parameters must be within an available window as returned by
    /// [`poll_available`](`SplittableSource::poll_available`).
    /// * The view must not overlap with any other view of this stream.
    ///
    /// Note that this function produces a mutable reference from a regular reference.
    /// Implementations of this function must take care to correctly implement interior
    /// mutability (such as using `UnsafeCell`).
    #[allow(clippy::mut_from_ref)]
    unsafe fn view_mut(&self, index: u64, len: usize) -> &mut [Self::Item];
}
