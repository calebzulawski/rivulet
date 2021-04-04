#![cfg_attr(docsrs, feature(doc_cfg))]
//! Rivulet provides tools for creating and processing asynchronous streams of contiguous data.

mod base;
pub use base::*;

#[cfg(feature = "buffer")]
#[cfg_attr(docsrs, doc(cfg(feature = "buffer")))]
pub mod buffer;

#[cfg(feature = "io")]
#[cfg_attr(docsrs, doc(cfg(feature = "io")))]
pub mod io;
