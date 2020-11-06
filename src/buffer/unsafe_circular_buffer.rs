use num_integer::{div_ceil, lcm};
use std::mem::{size_of, MaybeUninit};

pub struct UnsafeCircularBuffer<T> {
    ptr: *mut T,
    size: usize,
}

unsafe impl<T> Send for UnsafeCircularBuffer<T> where T: Send {}
unsafe impl<T> Sync for UnsafeCircularBuffer<T> where T: Send {}

impl<T> Drop for UnsafeCircularBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            crate::buffer::vm::deallocate_mirrored(self.ptr as *mut u8, self.size * size_of::<T>())
                .unwrap();
        }
    }
}

impl<T: Default> UnsafeCircularBuffer<T> {
    pub fn new(minimum_size: usize) -> Self {
        // Determine the smallest buffer larger than minimum_size that is both a multiple of the
        // allocation size and the type size.
        let size_bytes = {
            let granularity = lcm(crate::buffer::vm::allocation_size(), size_of::<T>());
            div_ceil(minimum_size * size_of::<T>(), granularity)
                .checked_mul(granularity)
                .unwrap()
        };
        let size = size_bytes / size_of::<T>();

        // Initialize the buffer memory
        let ptr = unsafe {
            let ptr = crate::buffer::vm::allocate_mirrored(size_bytes).unwrap() as *mut T;
            for v in std::slice::from_raw_parts_mut(ptr as *mut MaybeUninit<T>, size) {
                v.as_mut_ptr().write(T::default());
            }
            ptr
        };

        Self { ptr, size }
    }
}

impl<T> UnsafeCircularBuffer<T> {
    pub fn len(&self) -> usize {
        self.size
    }

    pub unsafe fn range(&self, offset: usize, size: usize) -> &[T] {
        debug_assert!(offset < self.len());
        debug_assert!(size <= self.len());
        std::slice::from_raw_parts(self.ptr.add(offset), size)
    }

    pub unsafe fn range_mut(&self, offset: usize, size: usize) -> &mut [T] {
        debug_assert!(offset < self.len());
        debug_assert!(size <= self.len());
        std::slice::from_raw_parts_mut(self.ptr.add(offset), size)
    }
}
