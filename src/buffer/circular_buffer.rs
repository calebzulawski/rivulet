use crate::{buffer::unsafe_circular_buffer::UnsafeCircularBuffer, Error, Sink, Source, SourceMut};
use futures::task::AtomicWaker;
use pin_project::pin_project;
use std::{
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, RwLock,
    },
    task::{Context, Poll},
};

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
    fn sink_impl(&mut self) -> &mut [T] {
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
        let reserved = circular_sub(self.tail, head, self.state.buffer.len());
        self.available = self.max_len() - reserved;
        self.available >= count
    }

    fn poll_reserve_impl(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        if count > self.max_len() {
            return Poll::Ready(Err(Error::Overflow));
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
            let closed = self.state.closed.load(Ordering::Relaxed);
            if self.data_available(count) {
                Poll::Ready(Ok(()))
            } else if closed {
                Poll::Ready(Err(Error::Closed))
            } else {
                Poll::Pending
            }
        }
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
        if count > self.available {
            return Poll::Ready(Err(Error::Overflow));
        }

        // Advance the buffer
        self.tail = circular_add(self.tail, count, self.state.buffer.len());
        self.available -= count;
        self.state.tail.store(self.tail, Ordering::Relaxed);
        self.state.readers.wake();

        Poll::Ready(Ok(()))
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
    fn source_impl(&self) -> &[T] {
        unsafe { self.state.buffer.range(self.head, self.available) }
    }

    fn data_available(&mut self, count: usize) -> bool {
        let tail = self.state.tail.load(Ordering::Relaxed);
        self.available = circular_sub(tail, self.head, self.state.buffer.len());
        self.available >= count
    }

    fn poll_request_impl(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        if count > (self.state.buffer.len() - 1) {
            return Poll::Ready(Err(Error::Overflow));
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
            if self.data_available(count) {
                Poll::Ready(Ok(()))
            } else if self.state.closed.load(Ordering::Relaxed) {
                Poll::Ready(Err(Error::Closed))
            } else {
                Poll::Pending
            }
        }
    }

    fn poll_consume_impl(
        mut self: Pin<&mut Self>,
        _: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        if count > self.available {
            return Poll::Ready(Err(Error::Overflow));
        }

        // Advance the head
        self.head = circular_add(self.head, count, self.state.buffer.len());
        self.available -= count;
        self.reader.head.store(self.head, Ordering::Relaxed);
        self.state.write_waker.wake();

        Poll::Ready(Ok(()))
    }
}

// If there is a single reader, it can obtain mutable access
impl<T> SourceImpl<T, Arc<SingleReader>> {
    fn source_mut_impl(&mut self) -> &mut [T] {
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
    pub fn buffer<T: Send + Sync + Default + 'static>(
        min_size: usize,
    ) -> (BufferSink<T>, BufferSource<T>) {
        assert!(min_size > 0, "`min_size` must be greater than 0");

        let reader = Arc::new(SingleReader {
            head: AtomicUsize::new(0),
            waker: AtomicWaker::new(),
        });

        let readers = RwLock::new(vec![reader.clone()]);

        let state = Arc::new(State::new(min_size, MultipleReaders { readers }));

        (
            BufferSink(SinkImpl {
                state: state.clone(),
                tail: 0,
                available: 0,
            }),
            BufferSource(SourceImpl {
                state,
                head: 0,
                available: 0,
                reader,
            }),
        )
    }

    /// Write values to the associated `BufferSource`s.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    pub struct BufferSink<T: Send + Sync + 'static>(#[pin] SinkImpl<T, MultipleReaders>);

    /// Read values from the associated `BufferSink`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    #[derive(Clone)]
    pub struct BufferSource<T>(#[pin] SourceImpl<T, MultipleReaders>);

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
    pub fn buffer<T: Send + Sync + Default + 'static>(
        min_size: usize,
    ) -> (BufferSink<T>, BufferSource<T>) {
        assert!(min_size > 0, "`min_size` must be greater than 0");

        let reader = Arc::new(SingleReader {
            head: AtomicUsize::new(0),
            waker: AtomicWaker::new(),
        });
        let state = Arc::new(State::new(min_size, reader.clone()));

        (
            BufferSink(SinkImpl {
                state: state.clone(),
                tail: 0,
                available: 0,
            }),
            BufferSource(SourceImpl {
                state,
                reader,
                head: 0,
                available: 0,
            }),
        )
    }

    /// Write values to the associated `BufferSource`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    pub struct BufferSink<T: Send + Sync + 'static>(#[pin] SinkImpl<T, Arc<SingleReader>>);

    /// Read values from the associated `BufferSink`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[pin_project]
    pub struct BufferSource<T>(#[pin] SourceImpl<T, Arc<SingleReader>>);

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
