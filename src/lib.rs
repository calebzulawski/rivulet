#![cfg_attr(docsrs, feature(doc_cfg))]
//! Rivulet provides tools for creating and processing asynchronous streams of contiguous data.

mod base;
pub use base::*;

pub mod error;

#[cfg(all(feature = "std", feature = "buffer"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std", feature = "buffer"))))]
pub mod buffer;

#[cfg(all(feature = "std", feature = "io"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "std", feature = "io"))))]
pub mod io;
