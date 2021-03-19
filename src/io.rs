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
/// Polling this future performs the buffering.  The future returns when the `Read` is finished or
/// on an error.
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
    /// Creates a new `ReadToSink` with a default commit size of 8KB.
    pub fn new(read: R, sink: S) -> Self {
        Self::with_commit_size(read, sink, 8192)
    }

    /// Creates a new `ReadToSink` with the specified commit size.
    pub fn with_commit_size(read: R, sink: S, commit_size: usize) -> Self {
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
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(Error::StreamError(e))),
                    _ => *pinned.reserved = true,
                }
            }

            // Write to the sink
            let buf = &mut pinned.sink.sink()[*pinned.position..];
            if !buf.is_empty() {
                match pinned.read.read(buf) {
                    Ok(count) => {
                        *pinned.position += count;

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

            // Commit to the sink
            if pinned.position >= pinned.block {
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
                    Poll::Ready(Err(e)) => return Poll::Ready(Err(Error::StreamError(e))),
                }
            }
        }
    }
}
