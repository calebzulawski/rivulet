//! Traits defining common stream interfaces.

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use pin_project::pin_project;

macro_rules! future {
    { $(#[$attr:meta])* $type:ident => $poll:ident => $error:ident} => {
        $(#[$attr])*
        #[pin_project]
        pub struct $type<'a, T> {
            #[pin]
            handle: &'a mut T,
            count: usize,
        }

        impl<'a, T> Future for $type<'a, T>
        where
            T: View + Unpin,
        {
            type Output = Result<(), T::$error>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
                let count = self.count;
                let pinned = self.project();
                pinned.handle.$poll(cx, count)
            }
        }
    }
}

future! {
    /// Future produced by [`View::grant`].
    Grant => poll_grant => GrantError
}
future! {
    /// Future produced by [`View::release`].
    Release => poll_release => ReleaseError
}

/// Obtain views into asynchronous contiguous-memory streams.
pub trait View {
    /// The streamed type.
    type Item;

    /// The error produced by [`poll_grant`](`Self::poll_grant`).
    type GrantError: core::fmt::Debug;

    /// The error produced by [`poll_release`](`Self::poll_release`).
    type ReleaseError: core::fmt::Debug;

    /// Obtain the current view of the stream.
    ///
    /// This view is obtained by successfully polling [`poll_grant`](`Self::poll_grant`) and
    /// advanced by successfully polling [`poll_release`](`Self::poll_release`).
    ///
    /// If this slice is smaller than the latest requested size, the end of the stream has been
    /// reached and no additional values will be provided.
    fn view(&self) -> &[Self::Item];

    /// Attempt to obtain a view of at least `count` elements.
    ///
    /// If the request exceeds the maximum possible grant (if there is one), an error should be returned.
    fn poll_grant(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::GrantError>>;

    /// Attempt to advance past the first `count` elements in the current view.
    ///
    /// # Panics
    /// If the request exceeds the current grant, this function should panic.
    fn poll_release(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::ReleaseError>>;

    /// Create a future that obtains a view of at least `count` elements.
    ///
    /// See [`poll_grant`](`Self::poll_grant`).
    fn grant(&mut self, count: usize) -> Grant<'_, Self>
    where
        Self: Sized + Unpin,
    {
        Grant {
            handle: self,
            count,
        }
    }

    /// Create a future that advances past the first `count` elements in the current view.
    ///
    /// See [`poll_release`](`Self::poll_release`).
    fn release(&mut self, count: usize) -> Release<'_, Self>
    where
        Self: Sized + Unpin,
    {
        Release {
            handle: self,
            count,
        }
    }

    /// Obtains a view of at least `count` elements, blocking the current thread.
    ///
    /// See [`poll_grant`](`View::poll_grant`).
    fn blocking_grant(&mut self, count: usize) -> Result<(), Self::GrantError>
    where
        Self: Sized + Unpin,
    {
        futures::executor::block_on(self.grant(count))
    }

    /// Advances past the first `count` elements in the current view, blocking the current thread.
    ///
    /// See [`poll_release`](`View::poll_release`).
    fn blocking_release(&mut self, count: usize) -> Result<(), Self::ReleaseError>
    where
        Self: Sized + Unpin,
    {
        futures::executor::block_on(self.release(count))
    }
}

impl<S: ?Sized + View + Unpin> View for &mut S {
    type Item = S::Item;
    type GrantError = S::GrantError;
    type ReleaseError = S::ReleaseError;

    fn view(&self) -> &[Self::Item] {
        View::view(*self)
    }

    fn poll_grant(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::GrantError>> {
        S::poll_grant(Pin::new(&mut **self), cx, count)
    }

    fn poll_release(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::ReleaseError>> {
        S::poll_release(Pin::new(&mut **self), cx, count)
    }
}

/// Obtain mutable views into asynchronous contiguous-memory mutable streams.
pub trait ViewMut: View {
    /// Obtain the current mutable view of the stream.
    ///
    /// Identical semantics to [`view`](trait.View.html#tymethod.view), but returns a mutable
    /// slice.
    fn view_mut(&mut self) -> &mut [Self::Item];
}

impl<S: ?Sized + ViewMut + Unpin> ViewMut for &mut S {
    fn view_mut(&mut self) -> &mut [Self::Item] {
        ViewMut::view_mut(*self)
    }
}

/// A marker trait that indicates this view is a source of data.
///
/// A newly granted view will read data from the stream.
pub trait Source: View {}

/// A marker trait that indicates this view is a sink of data.
///
/// Any data in a released view is committed to the stream.
pub trait Sink: ViewMut {}
