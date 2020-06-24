//! Rivulet provides tools for creating and processing asynchronous streams of contiguous data.

/// Create and manipulate asynchronous buffers.
pub mod buffer {
    pub use rivulet_buffer::*;
}

//pub use rivulet_core::transform;

pub use rivulet_core::stream::{Sink, Source};
