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
//! Let's create a simple averaging downsampler pipeline.
//!
//! ```
//! use rivulet::{View, ViewMut};
//! use futures::future::TryFutureExt;
//!
//! /// This function reads samples from the source pipeline, averages them,
//! /// and writes the average to the sink pipeline.
//! async fn downsample(
//!     mut source: impl View<Item=f32>,
//!     mut sink: impl ViewMut<Item=f32>,
//!     factor: usize
//! ) -> Result<(), &'static str> {
//!     loop {
//!         // Wait for the input and output to be available.
//!         tokio::try_join!(
//!             source.grant(factor).map_err(|_| "we got an input error!"),
//!             sink.grant(1).map_err(|_| "we got an output error!"),
//!         )?;
//!
//!         // Each view could be longer (if extra data is available)
//!         // or shorter (if the stream closed)
//!         let input = source.view();
//!         let output = sink.view_mut();
//!         if input.len() < factor || output.is_empty() {
//!             break Ok(())
//!         }
//!
//!         // Average the values
//!         output[0] = input[..factor].iter().sum::<f32>() / factor as f32;
//!
//!         // We've written all our data, so release our current views
//!         // and advance the streams
//!         source.release(factor);
//!         sink.release(1);
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
pub use view::{View, ViewMut};
