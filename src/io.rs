//! Utilities for working with [`std::io`].

use crate::{Sink, Source};
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

/// An error produced either by an I/O operation or by a stream interface.
#[derive(Debug)]
pub enum Error {
    IoError(std::io::Error),
    StreamError(crate::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::IoError(err) => writeln!(f, "{}", err),
            Self::StreamError(err) => writeln!(f, "{}", err),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::IoError(err) => err.source(),
            Self::StreamError(err) => err.source(),
        }
    }
}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::IoError(error)
    }
}

impl From<crate::Error> for Error {
    fn from(error: crate::Error) -> Self {
        Self::StreamError(error)
    }
}

/// Buffers a [`std::io::Read`] into a [`Sink`](`crate::Sink`).
///
/// Polling this future performs the buffering until one of the following occurs:
/// * the reader reaches the end of the stream
/// * the sink is closed
/// * an error occurs
#[pin_project]
pub struct ReadToSink<R, S>
where
    R: std::io::Read,
    S: Unpin + Sink<Item = u8>,
{
    read: R,
    #[pin]
    sink: S,
    block: usize,
    position: usize,
    reserved: bool,
}

impl<R, S> ReadToSink<R, S>
where
    R: std::io::Read,
    S: Unpin + Sink<Item = u8>,
{
    /// Creates a new `ReadToSink` with a default read buffer size of 8KB.
    pub fn new(read: R, sink: S) -> Self {
        Self::with_read_size(read, sink, 8192)
    }

    /// Creates a new unbuffered `ReadToSink`.
    ///
    /// Reads will occur with whatever buffer size is provided by the sink.
    pub fn unbuffered(read: R, source: S) -> Self {
        Self::with_read_size(read, source, 1)
    }

    /// Creates a new `ReadToSink` with the specified read buffer size.
    pub fn with_read_size(read: R, sink: S, commit_size: usize) -> Self {
        assert!(commit_size > 0, "commit_size must not be zero");
        Self {
            read,
            sink,
            block: commit_size,
            position: 0,
            reserved: false,
        }
    }
}

impl<R, S> Future for ReadToSink<R, S>
where
    R: std::io::Read,
    S: Unpin + Sink<Item = u8>,
{
    type Output = Result<(), Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut pinned = self.project();

        loop {
            // Reserve space in the buffer
            if !*pinned.reserved {
                match pinned.sink.as_mut().poll_reserve(cx, *pinned.block) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(crate::Error::Closed)) => return Poll::Ready(Ok(())),
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(Error::StreamError(e))),
                    _ => *pinned.reserved = true,
                }
            }

            // Write to the sink
            if *pinned.position == 0 {
                let buf = pinned.sink.sink();
                if !buf.is_empty() {
                    match pinned.read.read(buf) {
                        Ok(count) => {
                            *pinned.position = count;

                            // Check for EOF
                            if count == 0 {
                                *pinned.block = 0;
                            }
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::Interrupted {
                                return Poll::Pending;
                            } else {
                                return Poll::Ready(Err(Error::IoError(e)));
                            }
                        }
                    }
                }
            }

            // Commit to the sink
            match pinned.sink.as_mut().poll_commit(cx, *pinned.position) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(())) => {
                    *pinned.position = 0;
                    *pinned.reserved = false;

                    // If EOF and all data has been commited, terminate the future.
                    if *pinned.block == 0 {
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Err(crate::Error::Closed)) => return Poll::Ready(Ok(())),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(Error::StreamError(e))),
            }
        }
    }
}

/// Buffers a [`std::io::Write`] from a [`Source`](`crate::Source`).
///
/// Polling this future performs the buffering until one of the following occurs:
/// * the writer stops accepting data
/// * the source is closed
/// * an error occurs
#[pin_project]
pub struct WriteFromSource<W, S>
where
    W: std::io::Write,
    S: Unpin + Source<Item = u8>,
{
    write: W,
    #[pin]
    source: S,
    block: usize,
    position: usize,
    requested: bool,
}

impl<W, S> WriteFromSource<W, S>
where
    W: std::io::Write,
    S: Unpin + Source<Item = u8>,
{
    /// Creates a new `WriteFromSource` with a default write buffer size of 8KB.
    pub fn new(write: W, source: S) -> Self {
        Self::with_write_size(write, source, 8192)
    }

    /// Creates a new unbuffered `WriteFromSource`.
    ///
    /// Writes will occur with whatever buffer size is provided by the source.
    pub fn unbuffered(write: W, source: S) -> Self {
        Self::with_write_size(write, source, 1)
    }

    /// Creates a new `WriteFromSource` with the specified write buffer size.
    pub fn with_write_size(write: W, source: S, write_size: usize) -> Self {
        assert!(write_size > 0, "write_size must not be zero");
        Self {
            write,
            source,
            block: write_size,
            position: 0,
            requested: false,
        }
    }
}

impl<W, S> Future for WriteFromSource<W, S>
where
    W: std::io::Write,
    S: Unpin + Source<Item = u8>,
{
    type Output = Result<(), Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut pinned = self.project();

        loop {
            // Request data to write
            if !*pinned.requested {
                match pinned.source.as_mut().poll_request(cx, *pinned.block) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Err(crate::Error::Closed)) => return Poll::Ready(Ok(())),
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(Error::StreamError(e))),
                    _ => *pinned.requested = true,
                }
            }

            // Write from the source
            if *pinned.position == 0 {
                let buf = pinned.source.source();
                if !buf.is_empty() {
                    match pinned.write.write(buf) {
                        Ok(count) => {
                            *pinned.position = count;

                            // Check for finished writer
                            if count == 0 {
                                *pinned.block = 0;
                            }
                        }
                        Err(e) => {
                            if e.kind() == std::io::ErrorKind::Interrupted {
                                return Poll::Pending;
                            } else {
                                return Poll::Ready(Err(Error::IoError(e)));
                            }
                        }
                    }
                }
            }

            // Advance the source
            match pinned.source.as_mut().poll_consume(cx, *pinned.position) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(Ok(())) => {
                    *pinned.position = 0;
                    *pinned.requested = false;

                    // If writer finished and all data has been commited, terminate the future.
                    if *pinned.block == 0 {
                        return Poll::Ready(Ok(()));
                    }
                }
                Poll::Ready(Err(crate::Error::Closed)) => return Poll::Ready(Ok(())),
                Poll::Ready(Err(e)) => return Poll::Ready(Err(Error::StreamError(e))),
            }
        }
    }
}
