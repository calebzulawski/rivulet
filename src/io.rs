//! Utilities for working with [`std::io`].

use crate::{Sink, Source};
use futures::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use pin_project::pin_project;
use std::io::{BufRead, Read, Write};
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

impl<S> Read for Reader<S>
where
    S: Source<Item = u8> + Unpin,
    std::io::Error: From<S::Error>,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = if buf.len() <= self.0.view().len() {
            buf.len()
        } else {
            self.0.blocking_grant(1)?;
            buf.len().min(self.0.view().len())
        };
        buf[..len].copy_from_slice(&self.0.view()[..len]);
        self.0.release(len);
        Ok(len)
    }
}

impl<S> BufRead for Reader<S>
where
    S: Source<Item = u8> + Unpin,
    std::io::Error: From<S::Error>,
{
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        self.0.blocking_grant(1)?;
        Ok(self.0.view())
    }

    fn consume(&mut self, amt: usize) {
        self.0.release(amt);
    }
}

/// Implements `futures::io::AsyncRead` for a source.
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
    /// Create a new `AsyncReader`
    pub fn new(source: S) -> Self {
        Self { source, len: 0 }
    }

    /// Return the original `Source`
    pub fn into_inner(self) -> S {
        self.source
    }
}

impl<S> AsyncRead for AsyncReader<S>
where
    S: Source<Item = u8> + Unpin,
    std::io::Error: From<S::Error>,
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
                futures::ready!(pinned.source.as_mut().poll_grant(cx, 1))?;
                buf.len().min(pinned.source.view().len())
            };
            buf[..*pinned.len].copy_from_slice(&pinned.source.view()[..*pinned.len]);
        }
        pinned.source.as_mut().release(*pinned.len);
        Poll::Ready(Ok(std::mem::take(pinned.len))) // set len to 0
    }
}

impl<S> AsyncBufRead for AsyncReader<S>
where
    S: Source<Item = u8> + Unpin,
    std::io::Error: From<S::Error>,
{
    fn poll_fill_buf<'a>(
        self: Pin<&'a mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<&'a [u8]>> {
        let mut pinned = self.project();
        futures::ready!(pinned.source.as_mut().poll_grant(cx, 1))?;
        Poll::Ready(Ok(Pin::into_inner(pinned.source).view()))
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let mut pinned = self.project();
        pinned.source.as_mut().release(amt);
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

impl<S> Write for Writer<S>
where
    S: Sink<Item = u8> + Unpin,
    std::io::Error: From<S::Error>,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = if buf.len() <= self.0.view().len() {
            buf.len()
        } else {
            self.0.blocking_grant(1)?;
            buf.len().min(self.0.view().len())
        };
        self.0.view_mut()[..len].copy_from_slice(&buf[..len]);
        self.0.release(len);
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Implements `futures::io::AsyncWrite` for a sink.
#[pin_project]
#[derive(Copy, Clone, Debug)]
pub struct AsyncWriter<S>
where
    S: Sink<Item = u8> + Unpin,
{
    #[pin]
    sink: S,
    len: usize,
}

impl<S> AsyncWriter<S>
where
    S: Sink<Item = u8> + Unpin,
{
    /// Create a new `AsyncWriter`
    pub fn new(sink: S) -> Self {
        Self { sink, len: 0 }
    }

    /// Return the original `Sink`
    pub fn into_inner(self) -> S {
        self.sink
    }
}

impl<S> AsyncWrite for AsyncWriter<S>
where
    S: Sink<Item = u8> + Unpin,
    std::io::Error: From<S::Error>,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let mut pinned = self.project();
        if *pinned.len == 0 {
            *pinned.len = if buf.len() <= pinned.sink.view().len() {
                buf.len()
            } else {
                futures::ready!(pinned.sink.as_mut().poll_grant(cx, 1))?;
                buf.len().min(pinned.sink.view().len())
            };
            pinned.sink.view_mut()[..*pinned.len].copy_from_slice(&buf[..*pinned.len]);
        }
        pinned.sink.as_mut().release(*pinned.len);
        Poll::Ready(Ok(std::mem::take(pinned.len))) // set to 0
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
