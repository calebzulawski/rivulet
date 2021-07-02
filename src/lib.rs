#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
//! Rivulet provides tools for creating and processing asynchronous streams of contiguous data.

mod base;
pub use base::*;

pub mod circular_buffer;
pub mod error;
pub mod io;
pub mod slice;
