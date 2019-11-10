//! Tasks for transforming streams element by element.

use crate::stream::{Sink, Source};

/// Transform items from a source into a sink.
///
/// The stream is operated on one memory page at a time.
pub async fn transform<T, U, SourceT, SinkU, F>(source: SourceT, sink: SinkU, f: F)
where
    SourceT: Source<Item = T> + Send,
    SinkU: Sink<Item = U> + Send,
    F: Fn(&T) -> U,
{
    transform_by(source, sink, f, region::page::size()).await
}

/// Transform items from a source into a sink.
///
/// The stream is operated on `size` items at a time.
pub async fn transform_by<T, U, SourceT, SinkU, F>(
    mut source: SourceT,
    mut sink: SinkU,
    f: F,
    size: usize,
) where
    SourceT: Source<Item = T> + Send,
    SinkU: Sink<Item = U> + Send,
    F: Fn(&T) -> U,
{
    while let Some(input) = source.next(size).await {
        if let Some(output) = sink.next(input.len()).await {
            assert_eq!(input.len(), output.len());
            for (i, o) in input.iter().zip(output.iter_mut()) {
                *o = f(i);
            }
        } else {
            break;
        }
    }
}

/// Transform chunks of items from a source into a sink.
///
/// Each chunk is one page of memory.
pub async fn transform_chunk<T, U, SourceT, SinkU, F>(source: SourceT, sink: SinkU, f: F)
where
    SourceT: Source<Item = T> + Send,
    SinkU: Sink<Item = U> + Send,
    F: Fn(&[T], &mut [U]),
{
    transform_chunk_by(source, sink, f, region::page::size()).await
}

/// Transform chunks of items from a source into a sink.
///
/// Each chunk is `size` items.
pub async fn transform_chunk_by<T, U, SourceT, SinkU, F>(
    mut source: SourceT,
    mut sink: SinkU,
    f: F,
    size: usize,
) where
    SourceT: Source<Item = T> + Send,
    SinkU: Sink<Item = U> + Send,
    F: Fn(&[T], &mut [U]),
{
    while let Some(input) = source.next(size).await {
        if let Some(output) = sink.next(input.len()).await {
            assert_eq!(input.len(), output.len());
            f(input, output);
        } else {
            break;
        }
    }
}
