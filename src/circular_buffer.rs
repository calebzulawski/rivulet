#![cfg(all(feature = "std"))]
#![cfg_attr(docsrs, doc(cfg(all(feature = "std"))))]
//! Asynchronous circular buffers.
//!
//! These buffers are optimized for contiguous memory segments and never copy data to other regions
//! of the buffer.
use crate::{error::GrantOverflow, View, ViewMut};
use futures::task::AtomicWaker;
use num_integer::{div_ceil, lcm};
use pin_project::pin_project;
use std::{
    mem::{size_of, MaybeUninit},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, RwLock,
    },
    task::{Context, Poll},
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

    pub unsafe fn range(&self, offset: usize, size: usize) -> &[T] {
        debug_assert!(offset < self.len());
        debug_assert!(size <= self.len());
        std::slice::from_raw_parts(self.ptr.add(offset), size)
    }

    // Only safe if you can guarantee no other references to the same range
    #[allow(clippy::mut_from_ref)]
    pub unsafe fn range_mut(&self, offset: usize, size: usize) -> &mut [T] {
        debug_assert!(offset < self.len());
        debug_assert!(size <= self.len());
        std::slice::from_raw_parts_mut(self.ptr.add(offset), size)
    }
}

// Shared state
struct State<T, R> {
    buffer: UnsafeCircularBuffer<T>,
    closed: AtomicBool,       // true if the stream is closed
    tail: AtomicUsize,        // maximum extent of written data
    write_waker: AtomicWaker, // waker stored by the writer when waiting
    readers: R,               // state of each attached reader
}

impl<T: Default, R> State<T, R> {
    fn new(minimum_size: usize, readers: R) -> Self {
        // The +1 ensures there's room for a marker element (to indicate the difference between
        // empty and full
        Self {
            buffer: UnsafeCircularBuffer::new(minimum_size + 1),
            closed: AtomicBool::new(false),
            tail: AtomicUsize::new(0),
            write_waker: AtomicWaker::new(),
            readers,
        }
    }
}

fn circular_add(a: usize, b: usize, len: usize) -> usize {
    (a + b) % len
}

fn circular_sub(a: usize, b: usize, len: usize) -> usize {
    (a + len - b) % len
}

struct SingleReader {
    waker: AtomicWaker,
    head: AtomicUsize,
}

struct MultipleReaders {
    readers: RwLock<Vec<Arc<SingleReader>>>,
}

trait Readers {
    /// Wake all waiting readers
    fn wake(&self);

    /// Load the earliest head of all readers
    fn load_head(&self, tail: usize, len: usize, ordering: Ordering) -> usize;

    /// Drop the provided reader from the list of readers, returning true if no readers remain
    fn drop_reader(&self, reader: &Arc<SingleReader>) -> bool;
}

impl Readers for Arc<SingleReader> {
    fn wake(&self) {
        self.waker.wake();
    }

    fn load_head(&self, _tail: usize, _len: usize, ordering: Ordering) -> usize {
        self.head.load(ordering)
    }

    fn drop_reader(&self, _reader: &Arc<SingleReader>) -> bool {
        true
    }
}

impl Readers for MultipleReaders {
    fn wake(&self) {
        let readers = self.readers.read().expect("another thread panicked!");
        for reader in readers.iter() {
            reader.waker.wake();
        }
    }

    fn load_head(&self, tail: usize, len: usize, ordering: Ordering) -> usize {
        let readers = self.readers.read().expect("another thread panicked!");
        let mut earliest_head = std::usize::MAX;
        let mut largest_distance = 0;
        for reader in readers.iter() {
            let head = reader.head.load(ordering);
            let distance = circular_sub(tail, head, len);
            if distance >= largest_distance {
                earliest_head = head;
                largest_distance = distance;
            }
        }
        assert!(earliest_head != std::usize::MAX);
        earliest_head
    }

    fn drop_reader(&self, reader: &Arc<SingleReader>) -> bool {
        let mut readers = self.readers.write().expect("another thread panicked!");
        readers.retain(|test_reader| !Arc::ptr_eq(test_reader, reader));
        readers.is_empty()
    }
}

// Implementation of a stream sink
struct SinkImpl<T, R>
where
    R: Readers,
{
    state: Arc<State<T, R>>,
    tail: usize,
    available: usize,
}

impl<T, R> Drop for SinkImpl<T, R>
where
    R: Readers,
{
    fn drop(&mut self) {
        self.state.closed.store(true, Ordering::Relaxed);
        self.state.readers.wake(); // waiting readers can exit without sufficient data
    }
}

impl<T, R> SinkImpl<T, R>
where
    R: Readers,
{
    fn view_impl(&self) -> &[T] {
        unsafe { self.state.buffer.range(self.tail, self.available) }
    }

    fn view_mut_impl(&mut self) -> &mut [T] {
        unsafe { self.state.buffer.range_mut(self.tail, self.available) }
    }

    fn max_len(&self) -> usize {
        // leave room for a single marker element, to allow distinguishing empty and full buffers
        self.state.buffer.len() - 1
    }

    fn data_available(&mut self, count: usize) -> bool {
        let head =
            self.state
                .readers
                .load_head(self.tail, self.state.buffer.len(), Ordering::Relaxed);
        let granted = circular_sub(self.tail, head, self.state.buffer.len());
        self.available = self.max_len() - granted;
        self.available >= count
    }

    fn poll_grant_impl(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), GrantOverflow>> {
        if count > self.max_len() {
            return Poll::Ready(Err(GrantOverflow(self.max_len())));
        }

        if self.available >= count {
            return Poll::Ready(Ok(()));
        }

        // Perform double-checking on the amount of available data
        // The first check is efficient, but may spuriously fail.
        // The second check occurs after the `acquire` produced by registering the waker.
        if self.data_available(count) {
            Poll::Ready(Ok(()))
        } else {
            self.state.write_waker.register(cx.waker());
            if self.data_available(count) || self.state.closed.load(Ordering::Relaxed) {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
    }

    fn release_impl(&mut self, count: usize) {
        if count == 0 {
            return;
        }

        assert!(
            count <= self.available,
            "attempted to release more than current grant"
        );

        // Advance the buffer
        self.tail = circular_add(self.tail, count, self.state.buffer.len());
        self.available -= count;
        self.state.tail.store(self.tail, Ordering::Relaxed);
        self.state.readers.wake();
    }
}

// Implementation of a stream sink
struct SourceImpl<T, R>
where
    R: Readers,
{
    state: Arc<State<T, R>>,
    reader: Arc<SingleReader>,
    head: usize,
    available: usize,
}

impl<T, R> Drop for SourceImpl<T, R>
where
    R: Readers,
{
    fn drop(&mut self) {
        if self.state.readers.drop_reader(&self.reader) {
            self.state.closed.store(true, Ordering::Relaxed);
        }
        self.state.write_waker.wake(); // writer might be able to advance or terminate
    }
}

// Readers can only be cloned if the stream supports multiple readers
impl<T> Clone for SourceImpl<T, MultipleReaders> {
    fn clone(&self) -> Self {
        let reader = Arc::new(SingleReader {
            waker: AtomicWaker::new(),
            head: AtomicUsize::new(self.head),
        });
        let mut readers = self
            .state
            .readers
            .readers
            .write()
            .expect("another thread panicked!");
        readers.push(reader.clone());

        Self {
            state: self.state.clone(),
            reader,
            head: self.head,
            available: self.available,
        }
    }
}

impl<T, R> SourceImpl<T, R>
where
    R: Readers,
{
    fn view_impl(&self) -> &[T] {
        unsafe { self.state.buffer.range(self.head, self.available) }
    }

    fn data_available(&mut self, count: usize) -> bool {
        let tail = self.state.tail.load(Ordering::Relaxed);
        self.available = circular_sub(tail, self.head, self.state.buffer.len());
        self.available >= count
    }

    fn poll_grant_impl(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), GrantOverflow>> {
        let max_len = self.state.buffer.len() - 1;
        if count > max_len {
            return Poll::Ready(Err(GrantOverflow(max_len)));
        }

        if self.available >= count {
            return Poll::Ready(Ok(()));
        }

        // Perform double-checking on the amount of available data
        // The first check is efficient, but may spuriously fail.
        // The second check occurs after the `acquire` produced by registering the waker.
        if self.data_available(count) {
            Poll::Ready(Ok(()))
        } else {
            self.reader.waker.register(cx.waker());
            if self.data_available(count) || self.state.closed.load(Ordering::Relaxed) {
                Poll::Ready(Ok(()))
            } else {
                Poll::Pending
            }
        }
    }

    fn release_impl(&mut self, count: usize) {
        if count == 0 {
            return;
        }

        assert!(
            count <= self.available,
            "attempted to release more than current grant"
        );

        // Advance the head
        self.head = circular_add(self.head, count, self.state.buffer.len());
        self.available -= count;
        self.reader.head.store(self.head, Ordering::Relaxed);
        self.state.write_waker.wake();
    }
}

// If there is a single reader, it can obtain mutable access
impl<T> SourceImpl<T, Arc<SingleReader>> {
    fn view_mut_impl(&mut self) -> &mut [T] {
        unsafe { self.state.buffer.range_mut(self.head, self.available) }
    }
}

/// A single-producer, multiple-consumer async circular buffer.
///
/// This buffer has readers that implement `Clone`, at the cost of locking.  For a lock-free buffer
/// see [`spsc`].
pub mod spmc {
    use super::*;

    /// Creates a single-producer, multiple-consumer async circular buffer.
    ///
    /// The buffer can store at least `min_size` elements, but might hold more.
    ///
    /// # Panics
    /// Panics if `min_size` is 0.
    pub fn buffer<T: Send + Sync + Default + 'static>(min_size: usize) -> (Sink<T>, Source<T>) {
        assert!(min_size > 0, "`min_size` must be greater than 0");

        let reader = Arc::new(SingleReader {
            head: AtomicUsize::new(0),
            waker: AtomicWaker::new(),
        });

        let readers = RwLock::new(vec![reader.clone()]);

        let state = Arc::new(State::new(min_size, MultipleReaders { readers }));

        (
            Sink(SinkImpl {
                state: state.clone(),
                tail: 0,
                available: 0,
            }),
            Source(SourceImpl {
                state,
                head: 0,
                available: 0,
                reader,
            }),
        )
    }

    /// Write values to the associated `Source`s.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    pub struct Sink<T: Send + Sync + 'static>(#[pin] SinkImpl<T, MultipleReaders>);

    /// Read values from the associated `Sink`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    #[derive(Clone)]
    pub struct Source<T>(#[pin] SourceImpl<T, MultipleReaders>);

    impl<T: Send + Sync + 'static> View for Sink<T> {
        type Item = T;
        type Error = GrantOverflow;

        fn view(&self) -> &[Self::Item] {
            self.0.view_impl()
        }

        fn poll_grant(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), GrantOverflow>> {
            let pinned = self.project();
            pinned.0.poll_grant_impl(cx, count)
        }

        fn release(&mut self, count: usize) {
            self.0.release_impl(count)
        }
    }

    impl<T: Send + Sync + 'static> ViewMut for Sink<T> {
        fn view_mut(&mut self) -> &mut [Self::Item] {
            self.0.view_mut_impl()
        }
    }

    impl<T> View for Source<T> {
        type Item = T;
        type Error = GrantOverflow;

        fn view(&self) -> &[Self::Item] {
            self.0.view_impl()
        }

        fn poll_grant(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), GrantOverflow>> {
            let pinned = self.project();
            pinned.0.poll_grant_impl(cx, count)
        }

        fn release(&mut self, count: usize) {
            self.0.release_impl(count)
        }
    }

    impl<T> crate::Source for Source<T> {}
    impl<T: Send + Sync + 'static> crate::Sink for Sink<T> {}
}

/// A single-producer, single-consumer async circular buffer.
///
/// This buffer is lock-free.
///
/// For a multiple-reader buffer see [`spmc`].
pub mod spsc {
    use super::*;

    /// Creates a single-producer, single-consumer async circular buffer.
    ///
    /// The buffer can store at least `min_size` elements, but might hold more.
    /// # Panics
    /// Panics if `min_size` is 0.
    pub fn buffer<T: Send + Sync + Default + 'static>(min_size: usize) -> (Sink<T>, Source<T>) {
        assert!(min_size > 0, "`min_size` must be greater than 0");

        let reader = Arc::new(SingleReader {
            head: AtomicUsize::new(0),
            waker: AtomicWaker::new(),
        });
        let state = Arc::new(State::new(min_size, reader.clone()));

        (
            Sink(SinkImpl {
                state: state.clone(),
                tail: 0,
                available: 0,
            }),
            Source(SourceImpl {
                state,
                reader,
                head: 0,
                available: 0,
            }),
        )
    }

    /// Write values to the associated `Source`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    pub struct Sink<T: Send + Sync + 'static>(#[pin] SinkImpl<T, Arc<SingleReader>>);

    /// Read values from the associated `Sink`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    pub struct Source<T>(#[pin] SourceImpl<T, Arc<SingleReader>>);

    impl<T: Send + Sync + 'static> View for Sink<T> {
        type Item = T;
        type Error = GrantOverflow;

        fn view(&self) -> &[Self::Item] {
            self.0.view_impl()
        }

        fn poll_grant(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), GrantOverflow>> {
            let pinned = self.project();
            pinned.0.poll_grant_impl(cx, count)
        }

        fn release(&mut self, count: usize) {
            self.0.release_impl(count)
        }
    }

    impl<T: Send + Sync + 'static> ViewMut for Sink<T> {
        fn view_mut(&mut self) -> &mut [Self::Item] {
            self.0.view_mut_impl()
        }
    }

    impl<T> View for Source<T> {
        type Item = T;
        type Error = GrantOverflow;

        fn view(&self) -> &[Self::Item] {
            self.0.view_impl()
        }

        fn poll_grant(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), GrantOverflow>> {
            let pinned = self.project();
            pinned.0.poll_grant_impl(cx, count)
        }

        fn release(&mut self, count: usize) {
            self.0.release_impl(count)
        }
    }

    impl<T> ViewMut for Source<T> {
        fn view_mut(&mut self) -> &mut [Self::Item] {
            self.0.view_mut_impl()
        }
    }

    impl<T> crate::Source for Source<T> {}
    impl<T: Send + Sync + 'static> crate::Sink for Sink<T> {}
}
