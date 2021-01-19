//! Rivulet provides tools for creating and processing asynchronous streams of contiguous data.

mod base;
pub use base::*;

#[cfg(feature = "buffer")]
pub mod buffer;
