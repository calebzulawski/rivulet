//! Traits defining common stream interfaces.

use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Debug)]
pub enum Error {
    Closed,
    Overflow,
    Other(Box<dyn std::error::Error>),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::Closed => writeln!(f, "the stream has been closed"),
            Self::Overflow => writeln!(f, "buffer overflow"),
            Self::Other(err) => writeln!(f, "{}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Other(ref err) => err.source(),
            _ => None,
        }
    }
}

/// The return status of a `Sink` or `Source`.
#[derive(Debug)]
pub enum Status<T> {
    /// The data is ready.
    Ready(T),
    /// The `Sink` or `Source` has terminated, but a partial slice might be available.
    Incomplete(T),
}

impl<T> Status<T> {
    /// Unwrap the `Ready` state, ignoring the `Incomplete` state.
    pub fn ready(self) -> Option<T> {
        match self {
            Self::Ready(v) => Some(v),
            _ => None,
        }
    }

    /// Unwraps the inner value regardless of status.
    pub fn into_inner(self) -> T {
        match self {
            Self::Ready(v) => v,
            Self::Incomplete(v) => v,
        }
    }
}

impl<'a, T> Status<&'a [T]> {
    /// Unwraps the inner slice if it isn't empty.
    pub fn nonempty(self) -> Option<&'a [T]> {
        let inner = self.into_inner();
        if inner.len() == 0 {
            None
        } else {
            Some(inner)
        }
    }
}

impl<'a, T> Status<&'a mut [T]> {
    /// Unwraps the inner slice if it isn't empty.
    pub fn nonempty(self) -> Option<&'a mut [T]> {
        let inner = self.into_inner();
        if inner.len() == 0 {
            None
        } else {
            Some(inner)
        }
    }
}

impl<T, U> AsRef<[T]> for Status<U>
where
    U: AsRef<[T]>,
{
    fn as_ref(&self) -> &[T] {
        match self {
            Self::Ready(v) => v.as_ref(),
            Self::Incomplete(v) => v.as_ref(),
        }
    }
}

impl<T, U> AsMut<[T]> for Status<U>
where
    U: AsMut<[T]>,
{
    fn as_mut(&mut self) -> &mut [T] {
        match self {
            Self::Ready(v) => v.as_mut(),
            Self::Incomplete(v) => v.as_mut(),
        }
    }
}

pub trait Sink {
    type Item;

    fn poll_reserve(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<&mut [Self::Item], Error>>;

    fn poll_commit(self: Pin<&mut Self>, cx: &mut Context, count: usize)
        -> Poll<Result<(), Error>>;
}

pub trait Source {
    type Item;

    fn poll_request(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<Status<&[Self::Item]>, Error>>;

    fn poll_consume(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;
}

pub trait SourceMut: Source {
    fn poll_request_mut(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<Status<&mut [Self::Item]>, Error>>;
}
