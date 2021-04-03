//! Utilities for working with [`std::io`].

use crate::{Sink, Source};
use pin_project::pin_project;
use std::future::Future;
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Implements `std::io::Read` for a source.
#[pin_project]
#[derive(Copy, Clone, Debug, Default, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct Reader<S>(#[pin] S)
where
    S: Source<Item = u8> + Unpin;

impl<S> Reader<S>
where
    S: Source<Item = u8> + Unpin,
{
    /// Create a new `Reader`
    pub fn new(source: S) -> Self {
        Self(source)
    }

    /// Return the original `Source`
    pub fn into_inner(self) -> S {
        self.0
    }
}

impl<S> Source for Reader<S>
where
    S: Source<Item = u8> + Unpin,
{
    type Item = u8;

    fn source(&self) -> &[Self::Item] {
        self.0.source()
    }

    fn poll_request(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), crate::Error>> {
        let pinned = self.project();
        pinned.0.poll_request(cx, count)
    }

    fn poll_consume(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), crate::Error>> {
        let pinned = self.project();
        pinned.0.poll_consume(cx, count)
    }
}

impl<S> std::io::Read for Reader<S>
where
    S: Source<Item = u8> + Unpin,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = if buf.len() <= self.source().len() {
            buf.len()
        } else {
            self.blocking_request(1).map_err(crate::Error::to_io)?;
            buf.len().min(self.source().len())
        };
        buf[..len].copy_from_slice(&self.source()[..len]);
        self.blocking_consume(len).map_err(crate::Error::to_io)?;
        Ok(len)
    }
}
