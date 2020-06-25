//! Traits defining common stream interfaces.

pub use crate::stream_ext::*;

use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Debug)]
pub enum Error {
    Closed,
    Overflow,
    Other(Box<dyn std::error::Error + Send>),
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

pub trait Sink {
    type Item;

    fn sink(&mut self) -> &mut [Self::Item];

    fn poll_reserve(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;

    fn poll_commit(self: Pin<&mut Self>, cx: &mut Context, count: usize)
        -> Poll<Result<(), Error>>;
}

impl<S: ?Sized + Sink + Unpin> Sink for &mut S {
    type Item = S::Item;

    fn sink(&mut self) -> &mut [Self::Item] {
        Sink::sink(*self)
    }

    fn poll_reserve(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        S::poll_reserve(Pin::new(&mut **self), cx, count)
    }

    fn poll_commit(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        S::poll_commit(Pin::new(&mut **self), cx, count)
    }
}

pub trait Source {
    type Item;

    fn source(&self) -> &[Self::Item];

    fn poll_request(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;

    fn poll_consume(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>>;
}

impl<S: ?Sized + Source + Unpin> Source for &mut S {
    type Item = S::Item;

    fn source(&self) -> &[Self::Item] {
        Source::source(*self)
    }

    fn poll_request(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        S::poll_request(Pin::new(&mut **self), cx, count)
    }

    fn poll_consume(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        S::poll_consume(Pin::new(&mut **self), cx, count)
    }
}

pub trait SourceMut: Source {
    fn source_mut(&mut self) -> &mut [Self::Item];
}

impl<S: ?Sized + SourceMut + Unpin> SourceMut for &mut S {
    fn source_mut(&mut self) -> &mut [Self::Item] {
        SourceMut::source_mut(*self)
    }
}
