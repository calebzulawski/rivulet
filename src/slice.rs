//! Utilities for working with slices.

use crate::{Sink, Source, View, ViewMut};
use core::{
    convert::Infallible,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll},
};

/// Implements all of the stream traits on slices or slice-referenceable types.
pub struct Slice<T, V> {
    slice: T,
    offset: usize,
    _value: PhantomData<[V]>,
}

impl<T, V> Slice<T, V>
where
    T: AsRef<[V]>,
{
    /// Create a stream from a slice or slice-referenceable type.
    pub fn new(slice: T) -> Self {
        Self {
            slice,
            offset: 0,
            _value: PhantomData,
        }
    }

    /// Return the original type.
    pub fn into_inner(self) -> T {
        self.slice
    }
}

impl<T, V> View for Slice<T, V>
where
    T: AsRef<[V]>,
{
    type Item = V;
    type Error = Infallible;

    fn view(&self) -> &[Self::Item] {
        &self.slice.as_ref()[self.offset..]
    }

    fn poll_grant(
        self: Pin<&mut Self>,
        _cx: &mut Context,
        _count: usize,
    ) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }

    fn release(&mut self, count: usize) {
        assert!(
            count <= self.view().len(),
            "attempted to release more than current grant"
        );
        self.offset = self
            .offset
            .checked_add(count)
            .unwrap()
            .min(self.slice.as_ref().len())
    }
}

impl<T, V> ViewMut for Slice<T, V>
where
    T: AsRef<[V]> + AsMut<[V]>,
{
    fn view_mut(&mut self) -> &mut [Self::Item] {
        &mut self.slice.as_mut()[self.offset..]
    }
}

impl<T, V> Source for Slice<T, V> where T: AsRef<[V]> {}
impl<T, V> Sink for Slice<T, V> where T: AsRef<[V]> + AsMut<[V]> {}
