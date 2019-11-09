//! Traits defining common stream interfaces.

use async_trait::async_trait;

/// Provides a writable buffer of indeterminate length that can be written to in contiguous chunks.
#[async_trait]
pub trait Sink {
    type Item;

    /// Returns the length of the current slice into the writable buffer.
    fn len(&self) -> usize;

    /// Returns true if the current slice into the writable buffer is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Asynchronously consumes `advance` elements in the writable buffer and returns a slice to
    /// the next `size` elements.  Returns `None` if the stream has terminated and no more data is
    /// needed.
    ///
    /// `advance` is permitted to be less than `size`, returning an overlapping slice, but if
    /// `advance` is more than `size` the implementation may panic.
    async fn advance(&mut self, advance: usize, size: usize) -> Option<&mut [Self::Item]>;

    /// Asynchronously consumes the current slice into the writable buffer and returns the next
    /// slice into the buffer of size `size`.  Returns `None` if the stream has terminated and no
    /// more data is needed.
    async fn next(&mut self, size: usize) -> Option<&mut [Self::Item]> {
        let advance = self.len();
        self.advance(advance, size).await
    }

    /// Asynchronously resizes the current slice into the writable buffer without committing any
    /// elements.  Returns `None` if the stream has terminated and no more data is needed.
    async fn resize(&mut self, size: usize) -> Option<&mut [Self::Item]> {
        self.advance(0, size).await
    }
}

/// Provides a view into a stream of indeterminate length that can be read in contiguous chunks.
#[async_trait]
pub trait Source {
    type Item;

    /// Returns the length of the current slice into the stream.
    fn len(&self) -> usize;

    /// Returns true if the current slice into the stream is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Asynchronously advances the view into the stream by `advance` elements and returns a new
    /// slice of size `size`, or fewer elements if the stream has terminated.  Returns `None` if
    /// the stream has terminated and no more data is available.
    ///
    /// `advance` is permitted to be less than `size`, returning an overlapping slice, but if
    /// `advance` is more than `size` the implementation may panic.
    async fn advance(&mut self, advance: usize, size: usize) -> Option<&[Self::Item]>;

    /// Asynchronously advances past the current slice and returns a new slice of size `size`, or
    /// fewer elements if the stream has terminated.  Returns `None` if the stream has terminated
    /// and no more data is available.
    async fn next(&mut self, size: usize) -> Option<&[Self::Item]> {
        let advance = self.len();
        self.advance(advance, size).await
    }

    /// Asynchronously resizes the current slice into the stream to `size`, or fewer elements if
    /// the stream has terminated.  Returns `None` if the stream has terminated and no more
    /// elements are available.
    async fn resize(&mut self, size: usize) -> Option<&[Self::Item]> {
        self.advance(0, size).await
    }
}
