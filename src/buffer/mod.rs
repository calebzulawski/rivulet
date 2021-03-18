//! Asynchronous buffers for temporarily caching data.

/// Async circular buffers.
///
/// These buffers are optimized for contiguous memory segments and never copy data to other regions
/// of the buffer.
pub mod circular_buffer;

mod unsafe_circular_buffer;
