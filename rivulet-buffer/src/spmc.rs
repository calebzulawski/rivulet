//! A single-producer, multiple-consumer async buffer.

use crate::common::{spmc_buffer, SinkImpl, SourceImpl};
use async_trait::async_trait;
use rivulet_core::stream::{Sink, Source};

/// Creates a single-producer, multiple-consumer async buffer.
///
/// The buffer can store at least `min_size` elements, but might hold more.
/// # Panics
/// Panics if `min_size` is 0.
pub fn buffer<T: Send + Sync + Default + 'static>(
    min_size: usize,
) -> (BufferSink<T>, BufferSource<T>) {
    let (sink, source) = spmc_buffer(min_size);
    (BufferSink { sink }, BufferSource { source })
}

/// Write values to the associated `BufferSource`s.
///
/// Created by the [`buffer`] function.
///
/// [`buffer`]: fn.buffer.html
pub struct BufferSink<T: Send + Sync + 'static> {
    sink: SinkImpl<T>,
}

#[async_trait]
impl<T: Send + Sync + 'static> Sink for BufferSink<T> {
    type Item = T;

    fn len(&self) -> usize {
        self.sink.len()
    }

    async fn advance(&mut self, advance: usize, size: usize) -> Option<&mut [T]> {
        self.sink.advance(advance, size).await
    }
}

/// Read values from the associated `BufferSink`.
///
/// Created by the [`buffer`] function.
///
/// [`buffer`]: fn.buffer.html
#[derive(Clone)]
pub struct BufferSource<T> {
    source: SourceImpl<T>,
}

#[async_trait]
impl<T: Send + Sync + 'static> Source for BufferSource<T> {
    type Item = T;

    fn len(&self) -> usize {
        self.source.len()
    }

    async fn advance(&mut self, advance: usize, size: usize) -> Option<&[T]> {
        self.source.advance(advance, size).await
    }
}