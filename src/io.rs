//! Utilities for working with [`std::io`].

use crate::{Sink, Source};
use pin_project::pin_project;
use std::marker::Unpin;
use std::pin::Pin;
use std::task::{Context, Poll};

/// Implements [`std::io::Read`] for a source.
#[pin_project]
#[derive(Copy, Clone, Debug)]
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

impl<S> std::io::Read for Reader<S>
where
    S: Source<Item = u8> + Unpin,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = if buf.len() <= self.0.view().len() {
            buf.len()
        } else {
            self.0.blocking_grant(1).map_err(crate::Error::into_io)?;
            buf.len().min(self.0.view().len())
        };
        buf[..len].copy_from_slice(&self.0.view()[..len]);
        self.0.blocking_release(len).map_err(crate::Error::into_io)?;
        Ok(len)
    }
}

/// Implements [`futures::io::AsyncRead`] for a source.
#[pin_project]
#[derive(Copy, Clone, Debug)]
pub struct AsyncReader<S>
where
    S: Source<Item = u8> + Unpin,
{
    #[pin]
    source: S,
    len: usize,
}

impl<S> AsyncReader<S>
where
    S: Source<Item = u8> + Unpin,
{
    /// Create a new `Reader`
    pub fn new(source: S) -> Self {
        Self { source, len: 0 }
    }

    /// Return the original `Source`
    pub fn into_inner(self) -> S {
        self.source
    }
}

impl<S> futures::io::AsyncRead for AsyncReader<S>
where
    S: Source<Item = u8> + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut pinned = self.project();
        if *pinned.len == 0 {
            *pinned.len = if buf.len() <= pinned.source.view().len() {
                buf.len()
            } else {
                futures::ready!(pinned
                    .source
                    .as_mut()
                    .poll_grant(cx, 1)
                    .map_err(crate::Error::into_io))?;
                buf.len().min(pinned.source.view().len())
            };
            buf[..*pinned.len].copy_from_slice(&pinned.source.view()[..*pinned.len]);
        }
        pinned
            .source
            .as_mut()
            .poll_release(cx, *pinned.len)
            .map_ok(|_| std::mem::take(pinned.len)) // set to 0
            .map_err(crate::Error::into_io)
    }
}

/// Implements [`std::io::Write`] for a sink.
#[pin_project]
#[derive(Copy, Clone, Debug)]
pub struct Writer<S>(#[pin] S)
where
    S: Sink<Item = u8> + Unpin;

impl<S> Writer<S>
where
    S: Sink<Item = u8> + Unpin,
{
    /// Create a new `Writer`
    pub fn new(sink: S) -> Self {
        Self(sink)
    }

    /// Return the original `Sink`
    pub fn into_inner(self) -> S {
        self.0
    }
}

impl<S> std::io::Write for Writer<S>
where
    S: Sink<Item = u8> + Unpin,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = if buf.len() <= self.0.view().len() {
            buf.len()
        } else {
            self.0.blocking_grant(1).map_err(crate::Error::into_io)?;
            buf.len().min(self.0.view().len())
        };
        self.0.view_mut()[..len].copy_from_slice(&buf[..len]);
        self.0.blocking_release(len).map_err(crate::Error::into_io)?;
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
