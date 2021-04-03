//! Utilities for working with [`std::io`].

use crate::Source;
use pin_project::pin_project;
use std::marker::Unpin;

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
    pub fn new(stream: S) -> Self {
        Self(stream)
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
            self.0.blocking_grant(1).map_err(crate::Error::to_io)?;
            buf.len().min(self.0.view().len())
        };
        buf[..len].copy_from_slice(&self.0.view()[..len]);
        self.0.blocking_release(len).map_err(crate::Error::to_io)?;
        Ok(len)
    }
}
