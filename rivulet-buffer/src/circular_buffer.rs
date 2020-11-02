use crate::unsafe_circular_buffer::UnsafeCircularBuffer;
use futures::task::AtomicWaker;
use pin_project::pin_project;
use rivulet_core::stream::{Error, Sink, Source, SourceMut};
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex, RwLock,
    },
    task::{Context, Poll},
};

// Shared state
struct State<T, W> {
    buffer: UnsafeCircularBuffer<T>,
    head: AtomicUsize,
    tail: AtomicUsize,
    closed: AtomicBool,       // true if the stream is closed
    write_waker: AtomicWaker, // waker stored by the writer when waiting
    read_waker: W,            // waker(s) stored by the reader(s) when waiting
}

impl<T: Default, W> State<T, W> {
    fn new(minimum_size: usize, read_waker: W) -> Self {
        // The +1 ensures there's room for a marker element (to indicate the difference between
        // empty and full
        Self {
            buffer: UnsafeCircularBuffer::new(minimum_size + 1),
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
            write_waker: AtomicWaker::new(),
            read_waker,
        }
    }
}

impl<T, W> State<T, W> {
    // advance a buffer index by an amount (such as a head or tail)
    fn advance_value(&self, advance: usize, value: &AtomicUsize) -> usize {
        let new_value = (value.load(Ordering::Acquire) + advance) % self.buffer.len();
        value.store(new_value, Ordering::Release);
        new_value
    }

    // compute the distance between two buffer indices
    fn distance(&self, first: usize, second: usize) -> usize {
        (second + self.buffer.len() - first) % self.buffer.len()
    }

    // return the number of writable elements
    // (leaving room for a single marker element, to allow distinguishing empty and full buffers)
    fn writable_len(&self) -> usize {
        self.distance(
            self.tail.load(Ordering::Acquire),
            self.head.load(Ordering::Acquire) + self.buffer.len() - 1,
        )
    }
}

// Wakes a single reader
struct SingleWaker(Arc<AtomicWaker>);

// Wakes multiple readers
struct MultipleWaker(Mutex<Vec<Arc<AtomicWaker>>>);

trait WakeAll {
    // wake all waiting readers
    fn wake(&self);

    // drop a reader from the stream, returning true if no readers remain
    fn drop(&self, waker: &Arc<AtomicWaker>) -> bool;
}

impl WakeAll for SingleWaker {
    fn wake(&self) {
        self.0.wake();
    }

    fn drop(&self, _: &Arc<AtomicWaker>) -> bool {
        true
    }
}

impl WakeAll for MultipleWaker {
    fn wake(&self) {
        let wakers = self.0.lock().expect("another thread panicked!");
        for waker in wakers.iter() {
            waker.wake();
        }
    }

    fn drop(&self, waker: &Arc<AtomicWaker>) -> bool {
        let mut wakers = self.0.lock().expect("another thread panicked!");
        wakers.retain(|x| !Arc::ptr_eq(x, waker));
        wakers.is_empty()
    }
}

// Implementation of a stream sink
struct SinkImpl<T, W>
where
    W: WakeAll,
{
    state: Arc<State<T, W>>,
    reserved: (usize, usize),
}

impl<T, W> Drop for SinkImpl<T, W>
where
    W: WakeAll,
{
    fn drop(&mut self) {
        self.state.closed.store(true, Ordering::Relaxed);
        self.state.read_waker.wake(); // waiting readers can exit without sufficient data
    }
}

impl<T, W> SinkImpl<T, W>
where
    W: WakeAll,
{
    fn sink_impl(&mut self) -> &mut [T] {
        unsafe {
            self.state
                .buffer
                .range_mut(self.reserved.0, self.reserved.1)
        }
    }

    fn poll_reserve_impl(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        if count > (self.state.buffer.len() - 1) {
            return Poll::Ready(Err(Error::Overflow));
        }

        // Wait until enough room to write
        if self.state.writable_len() < count {
            self.state.write_waker.register(cx.waker());

            // Check if closed after registering, so we don't miss any notifications.
            return if self.state.closed.load(Ordering::Relaxed) {
                Poll::Ready(Err(Error::Closed))
            } else {
                Poll::Pending
            };
        }

        // Update the reserved amount
        self.reserved = (self.state.tail.load(Ordering::Acquire), count);

        Poll::Ready(Ok(()))
    }

    fn poll_commit_impl(
        mut self: Pin<&mut Self>,
        _: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        if count == 0 {
            return Poll::Ready(Ok(()));
        }

        // Ensure that the commit is possible with the current reservation
        if count > self.reserved.1 {
            return Poll::Ready(Err(Error::Overflow));
        }

        // Advance the buffer
        self.reserved = (
            self.state.advance_value(count, &self.state.tail),
            self.reserved.1 - count,
        );

        // Notify source(s) that data is available
        self.state.read_waker.wake();

        Poll::Ready(Ok(()))
    }
}

// Tracks a single reader head
struct SingleHead;

// Tracks multiple reader heads
struct MultipleHead {
    head: Arc<AtomicUsize>,                    // This reader's head
    heads: Arc<RwLock<Vec<Arc<AtomicUsize>>>>, // All readers' heads
}

trait Head {
    // Get this reader's head index
    fn get_head<T, W>(&self, state: &State<T, W>) -> usize;

    // Perform on drop
    fn drop<T, W>(&self, _: &State<T, W>) {}

    // Advance this reader's head
    fn advance<T, W>(&self, count: usize, state: &State<T, W>);
}

impl Head for SingleHead {
    fn get_head<T, W>(&self, state: &State<T, W>) -> usize {
        state.head.load(Ordering::Acquire)
    }

    fn drop<T, W>(&self, _: &State<T, W>) {}

    fn advance<T, W>(&self, count: usize, state: &State<T, W>) {
        state.advance_value(count, &state.head);
    }
}

impl MultipleHead {
    fn update_global_head<T, W>(&self, state: &State<T, W>) {
        // Find the earliest head in the list
        let heads = self.heads.read().unwrap();
        assert!(!heads.is_empty());
        let current_head = state.head.load(Ordering::Acquire);
        let mut earliest_head = std::usize::MAX;
        let mut smallest_distance = std::usize::MAX;
        for head in heads.iter() {
            let this_head = head.load(Ordering::Acquire);
            let this_distance = state.distance(current_head, this_head);
            if this_distance < smallest_distance {
                earliest_head = this_head;
                smallest_distance = this_distance;
            }
        }

        // Discard up to the earliest head
        state.head.store(earliest_head, Ordering::Release);
    }
}

impl Head for MultipleHead {
    fn get_head<T, W>(&self, _: &State<T, W>) -> usize {
        self.head.load(Ordering::Acquire)
    }

    fn drop<T, W>(&self, state: &State<T, W>) {
        // Remove this reader's head
        {
            let mut heads = self.heads.write().expect("another thread panicked!");
            heads.retain(|x| !Arc::ptr_eq(x, &self.head));
        }

        // Try to advance the global head
        self.update_global_head(state);
    }

    fn advance<T, W>(&self, count: usize, state: &State<T, W>) {
        // Advance just this head
        state.advance_value(count, &self.head);

        // Update the global head
        self.update_global_head(state);
    }
}

// Implementation of a stream sink
struct SourceImpl<T, W, H>
where
    W: WakeAll,
    H: Head,
{
    state: Arc<State<T, W>>,
    waker: Arc<AtomicWaker>,   // this reader's waker
    requested: (usize, usize), // the current request (offset, size)
    head: H,                   // the reader(s)'s head(s)
}

impl<T, W, H> Drop for SourceImpl<T, W, H>
where
    W: WakeAll,
    H: Head,
{
    fn drop(&mut self) {
        // Release this source's waker, and close the buffer if all readers have dropped
        if self.state.read_waker.drop(&self.waker) {
            self.state.closed.store(true, Ordering::Relaxed);
        }

        // Removing a reader may free up room to write, so wake the writer
        self.head.drop(&self.state);
        self.state.write_waker.wake();
    }
}

// Readers can only be cloned if the stream supports multiple readers
impl<T> Clone for SourceImpl<T, MultipleWaker, MultipleHead> {
    fn clone(&self) -> Self {
        // Copy the head value for the new source
        let head = Arc::new(AtomicUsize::new(self.head.head.load(Ordering::Acquire)));

        // Create a new waker
        let waker = Arc::new(AtomicWaker::new());

        let mut heads = self.head.heads.write().expect("another thread panicked!");
        let mut wakers = self
            .state
            .read_waker
            .0
            .lock()
            .expect("another thread panicked!");

        // Insert the head and waker for this reader
        heads.push(head.clone());
        wakers.push(waker.clone());

        Self {
            state: self.state.clone(),
            waker,
            requested: (0, 0),
            head: MultipleHead {
                head,
                heads: self.head.heads.clone(),
            },
        }
    }
}

impl<T, W, H> SourceImpl<T, W, H>
where
    H: Head + Unpin,
    W: WakeAll,
{
    fn source_impl(&self) -> &[T] {
        unsafe { self.state.buffer.range(self.requested.0, self.requested.1) }
    }

    fn poll_request_impl(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        if count > (self.state.buffer.len() - 1) {
            return Poll::Ready(Err(Error::Overflow));
        }

        let head = self.head.get_head(&self.state);

        // Wait for enough data to read
        // Stream status must be checked first, ensuring that `available` is the complete remainder
        let closed = self.state.closed.load(Ordering::Relaxed);
        let available = self
            .state
            .distance(head, self.state.tail.load(Ordering::Acquire));

        // Determine if we actually close the stream
        let requested = if available < count {
            self.waker.register(cx.waker());

            // Check if the sink disconnected after registering so we don't miss the notification.
            // Even if the connection is closed, we only want to error if there isn't enough data available.
            if closed {
                available
            } else {
                return Poll::Pending;
            }
        } else {
            count
        };

        // Update how much is requested
        self.requested = (head % self.state.buffer.len(), requested);

        Poll::Ready(if available < count {
            Err(Error::Closed)
        } else {
            Ok(())
        })
    }

    fn poll_consume_impl(
        mut self: Pin<&mut Self>,
        _: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        if count > self.requested.1 {
            return Poll::Ready(Err(Error::Overflow));
        }

        // Advance the head
        self.head.advance(count, &self.state);
        self.requested = (
            self.head.get_head(&self.state) % self.state.buffer.len(),
            self.requested.1 - count,
        );

        // Notify the writer
        self.state.write_waker.wake();

        Poll::Ready(Ok(()))
    }
}

// If there is a single reader, it can obtain mutable access
impl<T> SourceImpl<T, SingleWaker, SingleHead> {
    fn source_mut_impl(&self) -> &mut [T] {
        unsafe {
            self.state
                .buffer
                .range_mut(self.requested.0, self.requested.1)
        }
    }
}

/// A single-producer, multiple-consumer async circular buffer.
pub mod spmc {
    use super::*;

    /// Creates a single-producer, multiple-consumer async circular buffer.
    ///
    /// The buffer can store at least `min_size` elements, but might hold more.
    ///
    /// # Panics
    /// Panics if `min_size` is 0.
    pub fn buffer<T: Send + Sync + Default + 'static>(
        min_size: usize,
    ) -> (BufferSink<T>, BufferSource<T>) {
        assert!(min_size > 0, "`min_size` must be greater than 0");
        let waker = Arc::new(AtomicWaker::new());
        let wakers = Mutex::new(vec![waker.clone()]);
        let state = Arc::new(State::new(min_size, MultipleWaker(wakers)));

        let head = Arc::new(AtomicUsize::new(0));
        let heads = Arc::new(RwLock::new(vec![head.clone()]));

        (
            BufferSink(SinkImpl {
                state: state.clone(),
                reserved: (0, 0),
            }),
            BufferSource(SourceImpl {
                state,
                waker,
                requested: (0, 0),
                head: MultipleHead { head, heads },
            }),
        )
    }

    /// Write values to the associated `BufferSource`s.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    pub struct BufferSink<T: Send + Sync + 'static>(#[pin] SinkImpl<T, MultipleWaker>);

    /// Read values from the associated `BufferSink`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    #[derive(Clone)]
    pub struct BufferSource<T>(#[pin] SourceImpl<T, MultipleWaker, MultipleHead>);

    impl<T: Send + Sync + 'static> Sink for BufferSink<T> {
        type Item = T;

        fn sink(&mut self) -> &mut [Self::Item] {
            self.0.sink_impl()
        }

        fn poll_reserve(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Error>> {
            let pinned = self.project();
            pinned.0.poll_reserve_impl(cx, count)
        }

        fn poll_commit(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Error>> {
            let pinned = self.project();
            pinned.0.poll_commit_impl(cx, count)
        }
    }

    impl<T> Source for BufferSource<T> {
        type Item = T;

        fn source(&self) -> &[Self::Item] {
            self.0.source_impl()
        }

        fn poll_request(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Error>> {
            let pinned = self.project();
            pinned.0.poll_request_impl(cx, count)
        }

        fn poll_consume(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Error>> {
            let pinned = self.project();
            pinned.0.poll_consume_impl(cx, count)
        }
    }
}

/// A single-producer, single-consumer async circular buffer.
pub mod spsc {
    use super::*;

    /// Creates a single-producer, single-consumer async circular buffer.
    ///
    /// The buffer can store at least `min_size` elements, but might hold more.
    /// # Panics
    /// Panics if `min_size` is 0.
    pub fn buffer<T: Send + Sync + Default + 'static>(
        min_size: usize,
    ) -> (BufferSink<T>, BufferSource<T>) {
        assert!(min_size > 0, "`min_size` must be greater than 0");
        let waker = Arc::new(AtomicWaker::new());
        let state = Arc::new(State::new(min_size, SingleWaker(waker.clone())));

        (
            BufferSink(SinkImpl {
                state: state.clone(),
                reserved: (0, 0),
            }),
            BufferSource(SourceImpl {
                state,
                waker,
                requested: (0, 0),
                head: SingleHead,
            }),
        )
    }

    /// Write values to the associated `BufferSource`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    pub struct BufferSink<T: Send + Sync + 'static>(#[pin] SinkImpl<T, SingleWaker>);

    /// Read values from the associated `BufferSink`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    pub struct BufferSource<T>(#[pin] SourceImpl<T, SingleWaker, SingleHead>);

    impl<T: Send + Sync + 'static> Sink for BufferSink<T> {
        type Item = T;

        fn sink(&mut self) -> &mut [Self::Item] {
            self.0.sink_impl()
        }

        fn poll_reserve(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Error>> {
            let pinned = self.project();
            pinned.0.poll_reserve_impl(cx, count)
        }

        fn poll_commit(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Error>> {
            let pinned = self.project();
            pinned.0.poll_commit_impl(cx, count)
        }
    }

    impl<T> Source for BufferSource<T> {
        type Item = T;

        fn source(&self) -> &[Self::Item] {
            self.0.source_impl()
        }

        fn poll_request(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Error>> {
            let pinned = self.project();
            pinned.0.poll_request_impl(cx, count)
        }

        fn poll_consume(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<(), Error>> {
            let pinned = self.project();
            pinned.0.poll_consume_impl(cx, count)
        }
    }

    impl<T> SourceMut for BufferSource<T> {
        fn source_mut(&mut self) -> &mut [Self::Item] {
            self.0.source_mut_impl()
        }
    }
}
