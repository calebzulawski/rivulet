#![cfg(all(feature = "std"))]
#![cfg_attr(docsrs, doc(cfg(all(feature = "std"))))]
//! Asynchronous circular buffers.
//!
//! These buffers are optimized for contiguous memory segments and never copy data to other regions
//! of the buffer.
use crate::{
    error::GrantOverflow,
    splittable::{SplittableImpl, SplittableImplMut},
    View, ViewMut,
};
use futures::task::AtomicWaker;
use num_integer::{div_ceil, lcm};
use std::{
    convert::TryInto,
    mem::{size_of, MaybeUninit},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    task::{Context, Poll, Waker},
};

struct UnsafeCircularBuffer<T> {
    ptr: *mut T,
    size: usize,
}

unsafe impl<T> Send for UnsafeCircularBuffer<T> where T: Send {}
unsafe impl<T> Sync for UnsafeCircularBuffer<T> where T: Send {}

impl<T> Drop for UnsafeCircularBuffer<T> {
    fn drop(&mut self) {
        unsafe {
            for i in 0..self.size {
                std::ptr::drop_in_place(self.ptr.add(i));
            }
            vmap::os::unmap_ring(self.ptr as *mut u8, self.size * size_of::<T>()).unwrap();
        }
    }
}

impl<T: Default> UnsafeCircularBuffer<T> {
    pub fn new(minimum_size: usize) -> Self {
        // Determine the smallest buffer larger than minimum_size that is both a multiple of the
        // allocation size and the type size.
        let size_bytes = {
            let granularity = lcm(vmap::allocation_size(), size_of::<T>());
            div_ceil(minimum_size * size_of::<T>(), granularity)
                .checked_mul(granularity)
                .unwrap()
        };
        let size = size_bytes / size_of::<T>();

        // Initialize the buffer memory
        let ptr = unsafe {
            let ptr = vmap::os::map_ring(size_bytes).unwrap() as *mut T;
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

    // Only safe if you can guarantee no mutable references to this range
    pub unsafe fn range(&self, index: u64, len: usize) -> &[T] {
        debug_assert!(len <= self.len());
        let buf_len: u64 = self.len().try_into().unwrap();
        let offset = index % buf_len;
        std::slice::from_raw_parts(self.ptr.add(offset.try_into().unwrap()), len)
    }

    // Only safe if you can guarantee no other references to the same range
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn range_mut(&self, index: u64, len: usize) -> &mut [T] {
        debug_assert!(len <= self.len());
        let buf_len: u64 = self.len().try_into().unwrap();
        let offset = index % buf_len;
        std::slice::from_raw_parts_mut(self.ptr.add(offset.try_into().unwrap()), len)
    }
}

// Shared state
struct State<T> {
    buffer: UnsafeCircularBuffer<T>,
    closed: AtomicBool,          // true if the stream is closed
    head: AtomicU64,             // start index of written data
    tail: AtomicU64,             // start index of unwritten data
    write_waker: AtomicWaker,    // waker waited on by the writer
    wake_readers: Box<dyn Fn()>, // wake readers when new data is available
}

impl<T: Default> State<T> {
    fn new(minimum_size: usize) -> Self {
        // The +1 ensures there's room for a marker element (to indicate the difference between
        // empty and full
        Self {
            buffer: UnsafeCircularBuffer::new(minimum_size + 1),
            closed: AtomicBool::new(false),
            head: AtomicU64::new(0),
            tail: AtomicU64::new(0),
            write_waker: AtomicWaker::new(),
            wake_readers: Box::new(|| {}),
        }
    }
}

impl<T> State<T> {
    fn readable_len(&self, start: u64) -> usize {
        (self.tail.load(Ordering::Relaxed) - start)
            .try_into()
            .unwrap()
    }

    fn writeable_len(&self) -> usize {
        self.buffer.len() - self.readable_len(self.head.load(Ordering::Relaxed))
    }
}

pub struct Sink<T> {
    state: Arc<State<T>>,
    tail: u64,
    available: usize,
}

impl<T> Sink<T> {
    fn new(state: Arc<State<T>>) -> Self {
        Self {
            state,
            tail: 0,
            available: 0,
        }
    }
}

impl<T> Drop for Sink<T> {
    fn drop(&mut self) {
        self.state.closed.store(true, Ordering::Relaxed);
        (self.state.wake_readers)(); // waiting readers can exit without sufficient data
    }
}

impl<T> View for Sink<T> {
    type Item = T;
    type Error = GrantOverflow;

    fn view(&self) -> &[T] {
        unsafe { self.state.buffer.range(self.tail, self.available) }
    }

    fn poll_grant(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), GrantOverflow>> {
        if count > self.state.buffer.len() {
            return Poll::Ready(Err(GrantOverflow(self.state.buffer.len())));
        }

        if self.available >= count {
            return Poll::Ready(Ok(()));
        }

        // Perform double-checking on the amount of available data
        // The first check is efficient, but may spuriously fail.
        // The second check occurs after the `acquire` produced by registering the waker.
        if self.state.writeable_len() >= count {
            Poll::Ready(Ok(()))
        } else {
            self.state.write_waker.register(cx.waker());
            if self.state.writeable_len() >= count || self.state.closed.load(Ordering::Relaxed) {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
    }

    fn release(&mut self, count: usize) {
        if count == 0 {
            return;
        }

        assert!(
            count <= self.available,
            "attempted to release more than current grant"
        );

        // Advance the buffer
        self.available -= count;
        let count: u64 = count.try_into().unwrap();
        self.tail += count;
        self.state.tail.store(self.tail, Ordering::Relaxed);
        (self.state.wake_readers)();
    }
}

impl<T> ViewMut for Sink<T> {
    fn view_mut(&mut self) -> &mut [T] {
        unsafe { self.state.buffer.range_mut(self.tail, self.available) }
    }
}

impl<T> crate::Sink for Sink<T> {}

pub struct Source<T> {
    state: Arc<State<T>>,
}

impl<T> Source<T> {
    fn new(state: Arc<State<T>>) -> Self {
        Self { state }
    }
}

impl<T> Drop for Source<T> {
    fn drop(&mut self) {
        self.state.closed.store(true, Ordering::Relaxed);
        self.state.write_waker.wake();
    }
}

unsafe impl<T> SplittableImpl for Source<T> {
    type Item = T;
    type Error = GrantOverflow;

    unsafe fn set_reader_waker(&mut self, waker: impl Fn() + 'static) {
        self.state.wake_readers = Box::new(waker);
    }

    unsafe fn set_head(&mut self, index: u64) {
        self.state.head.store(index, Ordering::Relaxed);
    }

    unsafe fn compare_set_head(&self, index: u64) {
        // only set the head if it's greater than the current head
        let mut current = self.state.head.load(Ordering::Relaxed);
        if index > current {
            while let Err(previous) = self.state.head.compare_exchange_weak(
                current,
                index,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                if index > previous {
                    current = previous
                } else {
                    break;
                }
            }
        }
    }

    fn poll_available(
        self: Pin<&Self>,
        cx: &mut Context,
        register_wakeup: impl FnOnce(&Waker),
        index: u64,
        len: usize,
    ) -> Poll<Result<usize, Self::Error>> {
        let max_len = self.state.buffer.len();
        if len > max_len {
            return Poll::Ready(Err(GrantOverflow(max_len)));
        }

        // Perform double-checking on the amount of available data
        // The first check is efficient, but may spuriously fail.
        // The second check occurs after the `acquire` produced by registering the waker.
        let available = self.state.readable_len(index);
        if available >= len {
            Poll::Ready(Ok(available))
        } else {
            register_wakeup(cx.waker());
            let available = self.state.readable_len(index);
            if available >= len || self.state.closed.load(Ordering::Relaxed) {
                Poll::Ready(Ok(available))
            } else {
                Poll::Pending
            }
        }
    }

    unsafe fn view(&self, index: u64, len: usize) -> &[Self::Item] {
        self.state.buffer.range(index, len)
    }
}

impl<T> SplittableImplMut for Source<T> {
    unsafe fn view_mut(&self, index: u64, len: usize) -> &mut [Self::Item] {
        self.state.buffer.range_mut(index, len)
    }
}

pub fn buffer<T: Send + Sync + Default + 'static>(min_size: usize) -> (Sink<T>, Source<T>) {
    assert!(min_size > 0, "`min_size` must be greater than 0");

    let state = Arc::new(State::new(min_size));

    (Sink::new(state.clone()), Source::new(state))
}
