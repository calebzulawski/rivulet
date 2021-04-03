//! Traits defining common stream interfaces.

use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// An error produced when polling a [`Stream`](trait.Stream.html).
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
            T: Stream + Unpin,
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
    /// Future produced by [`Stream::grant`].
    Request => poll_grant
}
future! {
    /// Future produced by [`Stream::release`].
    Consume => poll_release
}

/// Interface for asynchronous contiguous-memory streams.
pub trait Stream {
    /// The type to be read.
    type Item;

    /// The buffer for reading data.
    ///
    /// This buffer is obtained by successfully polling [`poll_grant`](`Self::poll_grant`) and
    /// advanced by successfully polling [`poll_release`](`Self::poll_release`).
    fn stream(&self) -> &[Self::Item];

    /// Attempt to read at least `count` elements into the buffer.
    fn poll_grant(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;

    /// Attempt to advance past the first `count` elements in the buffer.
    fn poll_release(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;

    /// Create a future that reads at least `count` elements into the buffer.
    ///
    /// See [`poll_grant`](`Self::poll_grant`).
    fn grant(&mut self, count: usize) -> Request<'_, Self>
    where
        Self: Sized + Unpin,
    {
        Request {
            handle: self,
            count,
        }
    }

    /// Create a future that advances past the first `count` elements in the buffer.
    ///
    /// See [`poll_release`](`Self::poll_release`).
    fn release(&mut self, count: usize) -> Consume<'_, Self>
    where
        Self: Sized + Unpin,
    {
        Consume {
            handle: self,
            count,
        }
    }

    /// Reads at least `count` elements into the buffer, blocking the current thread.
    ///
    /// See [`poll_grant`](`Stream::poll_grant`).
    fn blocking_grant(&mut self, count: usize) -> Result<(), Error>
    where
        Self: Sized + Unpin,
    {
        futures::executor::block_on(self.grant(count))
    }

    /// Advances past the first `count` elements in the buffer, blocking the current thread.
    ///
    /// See [`poll_release`](`Stream::poll_release`).
    fn blocking_release(&mut self, count: usize) -> Result<(), Error>
    where
        Self: Sized + Unpin,
    {
        futures::executor::block_on(self.release(count))
    }
}

impl<S: ?Sized + Stream + Unpin> Stream for &mut S {
    type Item = S::Item;

    fn stream(&self) -> &[Self::Item] {
        Stream::stream(*self)
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

/// Interface for asynchronous contiguous-memory mutable streams.
pub trait StreamMut: Stream {
    /// The mutable buffer for reading data.
    ///
    /// Identical semantics to [`stream`](trait.Stream.html#tymethod.stream), but returns a mutable
    /// slice.
    fn stream_mut(&mut self) -> &mut [Self::Item];
}

impl<S: ?Sized + StreamMut + Unpin> StreamMut for &mut S {
    fn stream_mut(&mut self) -> &mut [Self::Item] {
        StreamMut::stream_mut(*self)
    }
}
