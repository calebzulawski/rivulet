//! Streams that can be split into multiple sources.

mod source;
pub use source::Source;

mod cloneable;
pub use cloneable::Cloneable;

pub mod implementation;
use implementation::IntoSplittableSource;

/// A source that can be split for use with multiple readers.
pub trait Splittable: IntoSplittableSource {
    /// Create a source for a single reader.
    fn into_source(self) -> Source<Self>
    where
        Self: Sized,
    {
        Source::new(self)
    }

    /// Create a source that implements `Clone`.
    fn into_cloneable_source(self) -> Cloneable<Self>
    where
        Self: Sized,
    {
        Cloneable::new(self)
    }
}

impl<T> Splittable for T where T: IntoSplittableSource {}
