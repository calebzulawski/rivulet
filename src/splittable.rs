//! Streams that can be split into multiple views.

use core::{
    pin::Pin,
    task::{Context, Poll, Waker},
};

mod view;
pub use view::View;

mod cloneable;
pub use cloneable::Cloneable;

mod sequence;
use sequence::make_sequence;
pub use sequence::{First, Second};

/// The implementation behind [`SplittableView`].
///
/// Unless you are manually implementing a view, you should use [`SplittableView`] directly.
///
/// # Safety
/// The implementation must satisfy the various interior mutability conditions specified in each method.
pub unsafe trait SplittableViewImpl: Sized + Unpin {
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
    unsafe fn set_reader_waker(&self, waker: impl Fn() + Send + Sync + 'static);

    /// Set the earliest position retained in the stream.
    ///
    /// # Panics
    /// May panic if the provided an index earlier than the current head.
    ///
    /// # Safety
    /// Only set the head if you have unique ownership of this and will not attempt to read any
    /// data earlier than `index`.
    unsafe fn set_head(&self, index: u64);

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
        register_wakeup: impl Fn(&Waker),
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

/// The implementation behind [`SplittableViewMut`].
///
/// Unless you are manually implementing a view, you should use [`SplittableViewMut`] directly.
///
/// # Safety
/// The implementation must satisfy the various interior mutability conditions specified in each method.
pub unsafe trait SplittableViewImplMut: SplittableViewImpl {
    /// Obtain a mutable view into the stream.
    ///
    /// # Safety
    /// * The parameters must be within an available window as returned by
    /// [`poll_available`](`SplittableViewImpl::poll_available`).
    /// * The view must not overlap with any other view of this stream.
    ///
    /// Note that this function produces a mutable reference from a regular reference.
    /// Implementations of this function must take care to correctly implement interior
    /// mutability (such as using `UnsafeCell`).
    #[allow(clippy::mut_from_ref)]
    unsafe fn view_mut(&self, index: u64, len: usize) -> &mut [Self::Item];
}

/// A view that can be split for use with multiple readers.
pub trait SplittableView: SplittableViewImpl {
    /// Create a view for a single reader.
    fn into_view(self) -> View<Self> {
        View::new(self)
    }

    /// Create a view that implements `Clone`.
    fn into_cloneable_view(self) -> Cloneable<Self> {
        Cloneable::new(self)
    }

    /// Split this view into two sequential views, such that data released by `First` becomes
    /// accessible to `Second`.
    fn sequence(self) -> (First<Self>, Second<Self>) {
        make_sequence(self)
    }
}

/// A mutable view that can be split for use with multiple readers.
pub trait SplittableViewMut: SplittableView + SplittableViewImplMut {}

impl<T> SplittableView for T where T: SplittableViewImpl {}
impl<T> SplittableViewMut for T where T: SplittableView + SplittableViewImplMut {}
