#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
//! Rivulet is a library for creating asynchronous pipelines of contiguous data.
//!
//! Main features, at a glance:
//!
//! * **Asynchronous**: Pipeline components are `async`-aware, allowing more control over task priority and data backpressure.
//! * **Contiguous views**: Data is always contiguous, accessible by a single slice.
//! * **Modular**: Pipelines can be combined and reused using common interfaces.
//!
//! # Example
//!
//! Let's create a simple pipeline for byte-reversing.
//!
//! ```
//! use rivulet::{Source, Sink};
//! use futures::future::TryFutureExt;
//!
//! async fn reverse_bytes(
//!     mut source: impl Source<Item=u8> + Unpin,
//!     mut sink: impl Sink<Item=u8> + Unpin,
//!     width: usize
//! ) -> Result<(), &'static str> {
//!     loop {
//!         // Wait for the input and output to have `width` elements available.
//!         tokio::try_join!(
//!             source.grant(width).map_err(|_| "we got an input error!"),
//!             sink.grant(width).map_err(|_| "we got an output error!"),
//!         )?;
//!
//!         // The view could be longer (if extra data is available) or shorter (if the stream closed)
//!         let input = source.view();
//!         let output = sink.view_mut();
//!         if input.len() < width || output.len() < width {
//!             break Ok(())
//!         }
//!         let input = &input[..width];
//!         let output = &mut output[..width];
//!
//!         // Copy the elements from the source into the sink, in reverse
//!         for (i, o) in input.iter().rev().zip(output.iter_mut()) {
//!             *o = *i;
//!         }
//!
//!         // We've written all our data, so release our current views and advance the streams
//!         source.release(width);
//!         sink.release(width);
//!     }
//! }
//! ```

pub mod circular_buffer;
pub mod error;
pub mod io;
pub mod lazy;
pub mod slice;
pub mod splittable;
pub mod view;

pub use circular_buffer::circular_buffer;
pub use splittable::Splittable;
pub use view::{Sink, Source, View, ViewMut};
