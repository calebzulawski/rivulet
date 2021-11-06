//! Utilities for working with slices.

use crate::splittable::{SplittableImpl, SplittableImplMut};
use core::{
    convert::Infallible,
    convert::TryInto,
    marker::PhantomData,
    pin::Pin,
    slice,
    task::{Context, Poll, Waker},
};

/// Treats a slice as a stream.
pub struct Slice<'a, T>(&'a [T]);

impl<'a, T> Slice<'a, T> {
    /// Create a stream from a slice.
    pub fn new(slice: &'a [T]) -> Self {
        Self(slice)
    }

    /// Return the original slice.
    pub fn into_inner(self) -> &'a [T] {
        self.0
    }
}

unsafe impl<'a, T> SplittableImpl for Slice<'a, T> {
    type Item = T;
    type Error = Infallible;

    unsafe fn set_reader_waker(&mut self, _: impl Fn() + Send + Sync + 'static) {}

    unsafe fn set_head(&mut self, _: u64) {}

    unsafe fn compare_set_head(&self, _: u64) {}

    fn poll_available(
        self: Pin<&Self>,
        _cx: &mut Context,
        _register_wakeup: impl FnOnce(&Waker),
        index: u64,
        _len: usize,
    ) -> Poll<Result<usize, Self::Error>> {
        let index: usize = index.try_into().unwrap();
        let len = self.0.as_ref().len();
        Poll::Ready(Ok(len - index.min(len)))
    }

    unsafe fn view(&self, index: u64, len: usize) -> &[Self::Item] {
        let index = index.try_into().unwrap();
        &self.0[index..index + len]
    }
}

/// Treats a mutable slice as a stream.
pub struct SliceMut<'a, T> {
    ptr: *mut T,
    len: usize,
    phantom_data: PhantomData<&'a mut [T]>,
}

impl<'a, T> SliceMut<'a, T> {
    /// Create a stream from a slice.
    pub fn new(slice: &'a mut [T]) -> Self {
        let ptr = slice.as_mut_ptr();
        let len = slice.len();
        Self {
            ptr,
            len,
            phantom_data: PhantomData,
        }
    }

    /// Return the original slice.
    pub fn into_inner(self) -> &'a mut [T] {
        // Safety: reconstruct the original slice
        unsafe { slice::from_raw_parts_mut(self.ptr, self.len) }
    }
}

unsafe impl<'a, T> SplittableImpl for SliceMut<'a, T> {
    type Item = T;
    type Error = Infallible;

    unsafe fn set_reader_waker(&mut self, _: impl Fn() + Send + Sync + 'static) {}

    unsafe fn set_head(&mut self, _: u64) {}

    unsafe fn compare_set_head(&self, _: u64) {}

    fn poll_available(
        self: Pin<&Self>,
        _cx: &mut Context,
        _register_wakeup: impl FnOnce(&Waker),
        index: u64,
        _len: usize,
    ) -> Poll<Result<usize, Self::Error>> {
        let index: usize = index.try_into().unwrap();
        Poll::Ready(Ok(self.len - index.min(self.len)))
    }

    unsafe fn view(&self, index: u64, len: usize) -> &[Self::Item] {
        slice::from_raw_parts(self.ptr.add(index.try_into().unwrap()) as *const T, len)
    }
}

unsafe impl<'a, T> SplittableImplMut for SliceMut<'a, T> {
    unsafe fn view_mut(&self, index: u64, len: usize) -> &mut [Self::Item] {
        slice::from_raw_parts_mut(self.ptr.add(index.try_into().unwrap()), len)
    }
}
