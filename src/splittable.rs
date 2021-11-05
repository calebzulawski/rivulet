//! Streams that can be split into multiple sources.

use core::{
    pin::Pin,
    task::{Context, Poll, Waker},
};

mod source;
pub use source::Source;

mod cloneable;
pub use cloneable::Cloneable;

/// The implementation behind [`Splittable`].
///
/// Unless you are manually implementing a source, you should use [`Splittable`] directly.
pub unsafe trait SplittableImpl {
    /// The streamed type.
    type Item;

    /// The error produced by [`poll_available`](`Self::poll_available`).
    type Error: core::fmt::Debug;

    /// Set the reader waking function.
    ///
    /// This should only be called once, after creation.
    ///
    /// # Safety
    /// Only set the waker if you have unique ownership of this.
    unsafe fn set_reader_waker(&mut self, waker: impl Fn() + Send + Sync + 'static);

    /// Set the earliest position retained in the stream.
    ///
    /// # Panics
    /// May panic if the provided an index earlier than the current head.
    ///
    /// # Safety
    /// Only set the head if you have unique ownership of this and will not attempt to read any
    /// data earlier than `index`.
    unsafe fn set_head(&mut self, index: u64);

    /// Set the earliest position retained in the stream.
    ///
    /// This function, unlike [`set_head`](`Self::set_head`), is synchronized between threads.
    /// If the provided head is less than the current head, the current head remains unchanged.
    ///
    /// # Safety
    /// See [`set_head`](`Self::set_head`).
    unsafe fn compare_set_head(&self, index: u64);

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

/// The implementation behind [`SplittableMut`].
///
/// Unless you are manually implementing a source, you should use [`SplittableMut`] directly.
pub trait SplittableImplMut: SplittableImpl {
    /// Obtain a mutable view into the stream.
    ///
    /// # Safety
    /// * The parameters must be within an available window as returned by
    /// [`poll_available`](`SplittableImpl::poll_available`).
    /// * The view must not overlap with any other view of this stream.
    ///
    /// Note that this function produces a mutable reference from a regular reference.
    /// Implementations of this function must take care to correctly implement interior
    /// mutability (such as using `UnsafeCell`).
    #[allow(clippy::mut_from_ref)]
    unsafe fn view_mut(&self, index: u64, len: usize) -> &mut [Self::Item];
}

/// A source that can be split for use with multiple readers.
pub trait Splittable: SplittableImpl {
    /// Create a source for a single reader.
    fn into_source(self) -> Source<Self>
    where
        Self: Sized,
    {
        Source::new(self)
    }

    /// Create a source that implements `Clone`.
    fn into_cloneable_source(self) -> Cloneable<Self>
    where
        Self: Sized,
    {
        Cloneable::new(self)
    }
}

/// A mutable source that can be split for use with multiple readers.
pub trait SplittableMut: Splittable + SplittableImplMut {}

impl<T> Splittable for T where T: SplittableImpl {}
impl<T> SplittableMut for T where T: Splittable + SplittableImplMut {}
