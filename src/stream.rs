//! Traits defining common stream interfaces.

use async_trait::async_trait;

/// Provides a mutable buffer into a stream of data, which can be consumed asynchronously.
#[async_trait]
pub trait Sink: Sized + Sync {
    type Item;

    /// Returns the writable buffer.
    fn as_slice(&self) -> & [Self::Item];

    /// Returns the writable buffer.
    fn as_slice_mut(&mut self) -> &mut [Self::Item];

    /// Returns the number of elements that can be written to the buffer.
    fn len(&self) -> usize;

    /// Returns `true` if the `Sink` contains no writable space.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Asynchronously consumes `advance` elements in the writable buffer and returns a new
    /// `Sink` with `size` elements, or `None` if the stream has terminated.
    async fn advance(self, advance: usize, size: usize) -> Option<Self>;

    /// Asynchronously consumes all elements in the writable buffer and returns a new `Sink`
    /// with `size` elements, or `None` if the stream has terminated.
    async fn next(self, size: usize) -> Option<Self> {
        let advance = self.len();
        self.advance(advance, size).await
    }

    /// Asynchronously resizes the current writable buffer to `size`, or returns `None` if the
    /// stream has terminated.
    async fn resize(self, size: usize) -> Option<Self> {
        self.advance(0, size).await
    }
}

/// Provides an immutable buffer into a stream of data, which can be read asynchronously.
#[async_trait]
pub trait Source: Sized + Sync {
    type Item;

    /// Returns the readable buffer.
    fn as_slice(&self) -> &[Self::Item];

    /// Returns the number of elements available to read in the buffer.
    fn len(&self) -> usize;

    /// Returns `true` if the `Source` contains no readable data.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Asynchronously skips `advance` elements in the stream and returns a new `Source` with
    /// `size` elements, or `None` if the stream has terminated.
    async fn advance(self, advance: usize, size: usize) -> Option<Self>;

    /// Asynchronously skips all elements in the current `Source` and returns a new `Source` with
    /// `size` elements, or `None` if the stream has terminated.
    async fn next(self, size: usize) -> Option<Self> {
        let advance = self.len();
        self.advance(advance, size).await
    }

    /// Asynchronously resizes the current readable buffer to `size`, or returns `None` if the
    /// stream has terminated.
    async fn resize(self, size: usize) -> Option<Self> {
        self.advance(0, size).await
    }
}
