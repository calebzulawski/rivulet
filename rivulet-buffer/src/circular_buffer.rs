use futures::{ready, task::AtomicWaker};
use pin_project::pin_project;
use rivulet_core::stream::{Error, Sink, Source, SourceMut, Status};
use slice_deque::Buffer;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::task::{Context, Poll};

// Shared state
struct State<T, W> {
    buffer: Buffer<T>,
    size: usize,              // number of elements in the buffer
    head: AtomicUsize,        // index of the first written element
    tail: AtomicUsize,        // index one past the end of the written data
    closed: AtomicBool,       // true if the stream is closed
    write_waker: AtomicWaker, // waker stored by the writer when waiting
    read_waker: W,            // waker(s) stored by the reader(s) when waiting
}

impl<T: Default, W> State<T, W> {
    fn new(min_size: usize, read_waker: W) -> Self {
        // Initialize the buffer memory
        // The +1 ensures there's room for a marker element (to indicate the difference between
        // empty and full
        let buffer = Buffer::<T>::uninitialized(2 * (min_size + 1)).unwrap();
        unsafe {
            for v in std::slice::from_raw_parts_mut(
                buffer.ptr() as *mut MaybeUninit<T>,
                buffer.len() / 2,
            ) {
                v.as_mut_ptr().write(T::default());
            }
        }

        Self {
            size: buffer.len() / 2,
            buffer,
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            closed: AtomicBool::new(false),
            write_waker: AtomicWaker::new(),
            read_waker,
        }
    }
}

impl<T, W> State<T, W> {
    // compute an offset into the buffer (absolute index)
    fn offset_ptr(&self, offset: usize) -> *const T {
        unsafe { self.buffer.ptr().add(offset % self.size) }
    }

    // return the pointer to the tail
    fn tail_ptr(&self) -> *mut T {
        unsafe { self.buffer.ptr().add(self.tail.load(Ordering::Relaxed)) }
    }

    // advance a buffer index by an amount (such as a head or tail)
    fn advance_value(&self, advance: usize, value: &AtomicUsize) {
        let new_value = (value.load(Ordering::Acquire) + advance) % self.size;
        value.store(new_value, Ordering::Release);
    }

    // compute the distance between two buffer indices
    fn distance(&self, first: usize, second: usize) -> usize {
        (second + self.size - first) % self.size
    }

    // return the number of writable elements
    // (leaving room for a single marker element, to allow distinguishing empty and full buffers)
    fn writable_len(&self) -> usize {
        self.distance(
            self.tail.load(Ordering::Acquire),
            self.head.load(Ordering::Acquire) + self.size - 1,
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
    reserved: Option<usize>, // number of reserved samples
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
    fn poll_reserve_impl(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<&mut [T], Error>> {
        if count > (self.state.size - 1) {
            return Poll::Ready(Err(Error::Overflow));
        }

        // Check if closed, optimistically
        if self.state.closed.load(Ordering::Relaxed) {
            return Poll::Ready(Err(Error::Closed));
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

        // Update how much is reserved
        self.reserved.replace(count);

        // Return the mutable slice
        let ptr = self.state.tail_ptr();
        Poll::Ready(Ok(unsafe { std::slice::from_raw_parts_mut(ptr, count) }))
    }

    fn poll_commit_impl(
        self: Pin<&mut Self>,
        _: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        if count == 0 {
            return Poll::Ready(Ok(()));
        }

        // Ensure that the commit is possible with the current reservation
        if let Some(reserved) = self.reserved {
            if count > reserved {
                return Poll::Ready(Err(Error::Overflow));
            }
        } else {
            panic!("cannot commit before reserving!");
        }

        // Advance the buffer
        self.state.advance_value(count, &self.state.tail);

        // Notify source(s) that data is available
        self.state.read_waker.wake();

        return Poll::Ready(Ok(()));
    }
}

// Tracks a single reader head
struct SingleHead;

// Tracks multiple reader heads
struct MultipleHead {
    head: Arc<AtomicUsize>,                   // This reader's head
    heads: Arc<Mutex<Vec<Arc<AtomicUsize>>>>, // All readers' heads
}

trait Head {
    // Get this reader's head index
    fn get_head<T, W>(&self, state: &State<T, W>) -> usize;

    // Advance this reader's head
    fn advance<T, W>(&self, count: usize, state: &State<T, W>);
}

impl Head for SingleHead {
    fn get_head<T, W>(&self, state: &State<T, W>) -> usize {
        state.head.load(Ordering::Acquire)
    }

    fn advance<T, W>(&self, count: usize, state: &State<T, W>) {
        state.advance_value(count, &state.head);
    }
}

impl Head for MultipleHead {
    fn get_head<T, W>(&self, _: &State<T, W>) -> usize {
        self.head.load(Ordering::Acquire)
    }

    fn advance<T, W>(&self, count: usize, state: &State<T, W>) {
        // Advance just this head
        state.advance_value(count, &self.head);

        // Find the earliest head in the list
        let heads = self.heads.lock().unwrap();
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

// Implementation of a stream sink
struct SourceImpl<T, W, H>
where
    W: WakeAll,
{
    state: Arc<State<T, W>>,
    waker: Arc<AtomicWaker>,  // this reader's waker
    requested: Option<usize>, // the current requested number of elements
    head: H,                  // the reader(s)'s head(s)
}

impl<T, W, H> Drop for SourceImpl<T, W, H>
where
    W: WakeAll,
{
    fn drop(&mut self) {
        // Release this source's waker, and close the buffer if all readers have dropped
        if self.state.read_waker.drop(&self.waker) {
            self.state.closed.store(true, Ordering::Relaxed);
            self.state.write_waker.wake();
        }
    }
}

// Readers can only be cloned if the stream supports multiple readers
impl<T> Clone for SourceImpl<T, MultipleWaker, MultipleHead> {
    fn clone(&self) -> Self {
        // Copy the head value for the new source
        let head = Arc::new(AtomicUsize::new(self.head.head.load(Ordering::Acquire)));

        // Create a new waker
        let waker = Arc::new(AtomicWaker::new());

        let mut heads = self.head.heads.lock().expect("another thread panicked!");
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
            requested: None,
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
    // requests a pointer
    fn poll_request_ptr(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(*mut T, usize), Error>> {
        if count <= (self.state.size - 1) {
            return Poll::Ready(Err(Error::Overflow));
        }

        let head = self.head.get_head(&self.state);

        // Wait for enough data to read
        let available = self
            .state
            .distance(head, self.state.tail.load(Ordering::Relaxed));
        if available < count {
            self.waker.register(cx.waker());

            // Check if the sink disconnected after registering so we don't miss the notification.
            // Also, even if the connection is closed, we only want to error if there isn't enough
            // data available.
            return if self.state.closed.load(Ordering::Relaxed) {
                Poll::Ready(Err(Error::Closed))
            } else {
                Poll::Pending
            };
        }

        self.requested.replace(count);

        Poll::Ready(Ok((self.state.offset_ptr(head) as _, available)))
    }

    // all readers support immutable requests
    fn poll_request_impl(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<Status<&[T]>, Error>> {
        let (ptr, available) = ready!(self.poll_request_ptr(cx, count))?;

        let slice = if available > 0 {
            unsafe { std::slice::from_raw_parts(ptr, available) }
        } else {
            &[]
        };

        // Return the slice
        Poll::Ready(Ok(if slice.len() < count {
            Status::Incomplete(slice)
        } else {
            Status::Ready(slice)
        }))
    }

    fn poll_consume_impl(
        self: Pin<&mut Self>,
        _: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Error>> {
        if let Some(requested) = self.requested {
            if count > requested {
                return Poll::Ready(Err(Error::Overflow));
            }
        } else {
            panic!("cannot consume before requesting!");
        }

        // Advance the head
        self.head.advance(count, &self.state);

        // Notify the reader
        self.state.write_waker.wake();

        Poll::Ready(Ok(()))
    }
}

// If there is a single reader, it can obtain mutable access
impl<T> SourceImpl<T, SingleWaker, SingleHead> {
    fn poll_request_mut_impl(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<Status<&mut [T]>, Error>> {
        let (ptr, available) = ready!(self.poll_request_ptr(cx, count))?;

        let slice = if available > 0 {
            unsafe { std::slice::from_raw_parts_mut(ptr, available) }
        } else {
            &mut []
        };

        // Return the slice
        Poll::Ready(Ok(if slice.len() < count {
            Status::Incomplete(slice)
        } else {
            Status::Ready(slice)
        }))
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
    pub fn circular_buffer<T: Send + Sync + Default + 'static>(
        min_size: usize,
    ) -> (BufferSink<T>, BufferSource<T>) {
        assert!(min_size > 0, "`min_size` must be greater than 0");
        let waker = Arc::new(AtomicWaker::new());
        let wakers = Mutex::new(vec![waker.clone()]);
        let state = Arc::new(State::new(min_size, MultipleWaker(wakers)));

        let head = Arc::new(AtomicUsize::new(0));
        let heads = Arc::new(Mutex::new(vec![head.clone()]));

        (
            BufferSink(SinkImpl {
                state: state.clone(),
                reserved: None,
            }),
            BufferSource(SourceImpl {
                state,
                waker,
                requested: None,
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

        fn poll_reserve(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<&mut [Self::Item], Error>> {
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

        fn poll_request(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<Status<&[Self::Item]>, Error>> {
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
    pub fn circular_buffer<T: Send + Sync + Default + 'static>(
        min_size: usize,
    ) -> (BufferSink<T>, BufferSource<T>) {
        assert!(min_size > 0, "`min_size` must be greater than 0");
        let waker = Arc::new(AtomicWaker::new());
        let state = Arc::new(State::new(min_size, SingleWaker(waker.clone())));

        (
            BufferSink(SinkImpl {
                state: state.clone(),
                reserved: None,
            }),
            BufferSource(SourceImpl {
                state,
                waker,
                requested: None,
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

        fn poll_reserve(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<&mut [Self::Item], Error>> {
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

        fn poll_request(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<Status<&[Self::Item]>, Error>> {
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
        fn poll_request_mut(
            self: Pin<&mut Self>,
            cx: &mut Context,
            count: usize,
        ) -> Poll<Result<Status<&mut [Self::Item]>, Error>> {
            let pinned = self.project();
            pinned.0.poll_request_mut_impl(cx, count)
        }
    }
}
