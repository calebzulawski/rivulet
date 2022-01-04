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
//! # Getting started
//!
//! The most basic interface is a [`View`].
//! Streams of data in `rivulet` are like slices, but you can't access the entire slice at once.
//! A [`View`] is sliding window over the stream, requiring only a small portion of the stream to
//! be in memory.
//!
//! A [`SplittableView`] is a special view that can split into multiple, simultaneously available views for use with multiple readers and writers.
//!
//! This crate provides a few stream implementations, but the most notable is the [`mod@circular_buffer`], which is optimized for asynchronous contiguous data access.
//!
//! # Example
//!
//! Let's create a simple averaging downsampler pipeline.
//!
//! ```
//! use rivulet::{View, ViewMut};
//! use futures::future::TryFutureExt;
//!
//! /// This function reads samples from the source, averages them,
//! /// and writes the average to the sink.
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
pub use splittable::SplittableView;
pub use view::{View, ViewMut};
