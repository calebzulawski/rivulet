//! Buffers for temporarily caching data.

pub mod spsc;

use slice_deque::Buffer;
use std::sync::atomic::{AtomicUsize, Ordering};

struct State<T> {
    buffer: Buffer<T>,
    size: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl<T: Default> State<T> {
    fn new(min_size: usize) -> Self {
        // Initialize the buffer memory
        // The +1 ensures there's room for a marker element (to indicate the difference between
        // empty and full
        let buffer = Buffer::<T>::uninitialized(2 * (min_size + 1)).unwrap();
        unsafe {
            for index in 0..(buffer.len() / 2) {
                buffer.ptr().add(index).write(T::default());
            }
        }

        Self {
            size: buffer.len() / 2,
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
        }
    }
}

impl<T> State<T> {
    fn head_ptr(&self) -> *const T {
        unsafe { self.buffer.ptr().add(self.head.load(Ordering::Relaxed)) }
    }

    fn tail_ptr(&self) -> *mut T {
        unsafe { self.buffer.ptr().add(self.tail.load(Ordering::Relaxed)) }
    }

    fn advance_tail(&self, advance: usize) {
        let new_tail = (self.tail.load(Ordering::Relaxed) + advance) % self.size;
        self.tail.store(new_tail, Ordering::Relaxed);
    }

    fn advance_head(&self, advance: usize) {
        let new_head = (self.head.load(Ordering::Relaxed) + advance) % self.size;
        self.head.store(new_head, Ordering::Relaxed);
    }

    fn writable_len(&self) -> usize {
        self.size - self.readable_len() - 1
    }

    fn readable_len(&self) -> usize {
        (self.tail.load(Ordering::Relaxed) + self.size - self.head.load(Ordering::Relaxed))
            % self.size
    }
}
