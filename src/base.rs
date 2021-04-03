//! Traits defining common stream interfaces.

use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// An error produced when polling a [`View`](trait.View.html).
#[derive(Debug)]
pub enum Error {
    /// The stream is closed and cannot be accessed.
    Closed,

    /// The request is malformed and results in a buffer overflow.
    Overflow,

    /// An I/O error occurred.
    IO(std::io::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::Closed => writeln!(f, "the stream has been closed"),
            Self::Overflow => writeln!(f, "buffer overflow"),
            Self::IO(err) => writeln!(f, "{}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IO(ref err) => err.source(),
            _ => None,
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::IO(error)
    }
}

impl Error {
    pub(crate) fn to_io(self) -> std::io::Error {
        match self {
            Self::IO(e) => e,
            e => std::io::Error::new(std::io::ErrorKind::Other, Box::new(e)),
        }
    }
}

macro_rules! future {
    { $(#[$attr:meta])* $type:ident => $poll:ident } => {
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
            type Output = Result<(), Error>;

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
    Grant => poll_grant
}
future! {
    /// Future produced by [`View::release`].
    Release => poll_release
}

/// Obtain views into asynchronous contiguous-memory streams.
pub trait View {
    /// The streamed type.
    type Item;

    /// Obtain the current view of the stream.
    ///
    /// This view is obtained by successfully polling [`poll_grant`](`Self::poll_grant`) and
    /// advanced by successfully polling [`poll_release`](`Self::poll_release`).
    fn view(&self) -> &[Self::Item];

    /// Attempt to obtain a view of at least `count` elements.
    fn poll_grant(self: Pin<&mut Self>, cx: &mut Context, count: usize) -> Poll<Result<(), Error>>;

    /// Attempt to advance past the first `count` elements in the current view.
    fn poll_release(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;

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
    fn blocking_grant(&mut self, count: usize) -> Result<(), Error>
    where
        Self: Sized + Unpin,
    {
        futures::executor::block_on(self.grant(count))
    }

    /// Advances past the first `count` elements in the current view, blocking the current thread.
    ///
    /// See [`poll_release`](`View::poll_release`).
    fn blocking_release(&mut self, count: usize) -> Result<(), Error>
    where
        Self: Sized + Unpin,
    {
        futures::executor::block_on(self.release(count))
    }
}

impl<S: ?Sized + View + Unpin> View for &mut S {
    type Item = S::Item;

    fn view(&self) -> &[Self::Item] {
        View::view(*self)
    }

    fn poll_grant(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        S::poll_grant(Pin::new(&mut **self), cx, count)
    }

    fn poll_release(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
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

/// Obtain views with stable memory locations.
///
/// # Safety
/// The lifetime of the pointers returned by this trait must be at least as long as the subview
/// remains in the current view.
pub unsafe trait StableView: View {
    /// Obtain a stable subview of the current view, starting at `offset` elements from the start
    /// of the current view and spanning `len` elements.
    ///
    /// The subview has the following qualities:
    /// * It may not be written to.
    /// * Adjacent or overlapping subviews are not guaranteed to be adjacent or overlapping in
    /// memory.
    /// * Dereferencing the pointer after the view is advanced past the start of the subview is
    /// undefined behavior.
    ///
    /// # Panics
    /// Panics if the subview would extend beyond the current view.
    fn stable_view(&self, offset: usize, len: usize) -> *const Self::Item;
}
