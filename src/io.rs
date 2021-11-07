#![cfg(all(feature = "std"))]
#![cfg_attr(docsrs, doc(cfg(all(feature = "std"))))]
//! Utilities for working with [`std::io`].

use crate::{View, ViewMut};
use futures::io::{AsyncBufRead, AsyncRead, AsyncWrite};
use std::{
    io::{BufRead, Read, Write},
    pin::Pin,
    task::{Context, Poll},
};

/// Implements [`std::io::Read`] for a source.
#[derive(Copy, Clone, Debug)]
pub struct Reader<T>(T)
where
    T: View<Item = u8>;

impl<T> Reader<T>
where
    T: View<Item = u8>,
{
    /// Create a new `Reader`
    pub fn new(source: T) -> Self {
        Self(source)
    }

    /// Return the original `View`
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Read for Reader<T>
where
    T: View<Item = u8>,
    std::io::Error: From<T::Error>,
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

impl<T> BufRead for Reader<T>
where
    T: View<Item = u8>,
    std::io::Error: From<T::Error>,
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
#[derive(Copy, Clone, Debug)]
pub struct AsyncReader<T>
where
    T: View<Item = u8>,
{
    source: T,
    len: usize,
}

impl<T> AsyncReader<T>
where
    T: View<Item = u8>,
{
    /// Create a new `AsyncReader`
    pub fn new(source: T) -> Self {
        Self { source, len: 0 }
    }

    /// Return the original `Source`
    pub fn into_inner(self) -> T {
        self.source
    }
}

impl<T> AsyncRead for AsyncReader<T>
where
    T: View<Item = u8>,
    std::io::Error: From<T::Error>,
{
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.len == 0 {
            self.len = if buf.len() <= self.source.view().len() {
                buf.len()
            } else {
                futures::ready!(Pin::new(&mut self.source).poll_grant(cx, 1))?;
                buf.len().min(self.source.view().len())
            };
            buf[..self.len].copy_from_slice(&self.source.view()[..self.len]);
        }
        let len = self.len;
        self.source.release(len);
        Poll::Ready(Ok(std::mem::take(&mut self.len))) // set len to 0
    }
}

impl<T> AsyncBufRead for AsyncReader<T>
where
    T: View<Item = u8>,
    std::io::Error: From<T::Error>,
{
    fn poll_fill_buf<'a>(
        mut self: Pin<&'a mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<std::io::Result<&'a [u8]>> {
        futures::ready!(Pin::new(&mut self.source).poll_grant(cx, 1))?;
        Poll::Ready(Ok(Pin::into_inner(self).source.view()))
    }

    fn consume(mut self: Pin<&mut Self>, amt: usize) {
        self.source.release(amt);
    }
}

/// Implements [`std::io::Write`] for a [`ViewMut`].
#[derive(Copy, Clone, Debug)]
pub struct Writer<T>(T)
where
    T: ViewMut<Item = u8>;

impl<T> Writer<T>
where
    T: ViewMut<Item = u8>,
{
    /// Create a new `Writer`
    pub fn new(sink: T) -> Self {
        Self(sink)
    }

    /// Return the original [`ViewMut`]
    pub fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Write for Writer<T>
where
    T: ViewMut<Item = u8>,
    std::io::Error: From<T::Error>,
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

/// Implements `futures::io::AsyncWrite` for a [`ViewMut`].
#[derive(Copy, Clone, Debug)]
pub struct AsyncWriter<T>
where
    T: ViewMut<Item = u8>,
{
    sink: T,
    len: usize,
}

impl<T> AsyncWriter<T>
where
    T: ViewMut<Item = u8>,
{
    /// Create a new `AsyncWriter`
    pub fn new(sink: T) -> Self {
        Self { sink, len: 0 }
    }

    /// Return the original [`ViewMut`]
    pub fn into_inner(self) -> T {
        self.sink
    }
}

impl<T> AsyncWrite for AsyncWriter<T>
where
    T: ViewMut<Item = u8>,
    std::io::Error: From<T::Error>,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        if self.len == 0 {
            self.len = if buf.len() <= self.sink.view().len() {
                buf.len()
            } else {
                futures::ready!(Pin::new(&mut self.sink).poll_grant(cx, 1))?;
                buf.len().min(self.sink.view().len())
            };
            let len = self.len;
            self.sink.view_mut()[..len].copy_from_slice(&buf[..len]);
        }
        let len = self.len;
        self.sink.release(len);
        Poll::Ready(Ok(std::mem::take(&mut self.len))) // set to 0
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}
