use futures::task::AtomicWaker;
use rivulet_core::stream::{Error, Status};
use slice_deque::Buffer;
use std::mem::MaybeUninit;
use std::pin::Pin;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::task::{Context, Poll};

struct State<T, W> {
    buffer: Buffer<T>,
    size: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
    closed: AtomicBool,
    write_waker: AtomicWaker,
    read_waker: W,
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
    fn offset_ptr(&self, offset: usize) -> *const T {
        unsafe { self.buffer.ptr().add(offset % self.size) }
    }

    fn tail_ptr(&self) -> *mut T {
        unsafe { self.buffer.ptr().add(self.tail.load(Ordering::Relaxed)) }
    }

    fn advance_value(&self, advance: usize, value: &AtomicUsize) {
        let new_value = (value.load(Ordering::Acquire) + advance) % self.size;
        value.store(new_value, Ordering::Release);
    }

    fn distance(&self, first: usize, second: usize) -> usize {
        (second + self.size - first) % self.size
    }

    fn writable_len(&self) -> usize {
        self.distance(
            self.tail.load(Ordering::Acquire),
            self.head.load(Ordering::Acquire) + self.size - 1,
        )
    }
}

struct SingleWaker(Arc<AtomicWaker>);

struct MultipleWaker(Mutex<Vec<Arc<AtomicWaker>>>);

trait WakeAll {
    fn wake(&self);

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

struct SinkImpl<T, W>
where
    W: WakeAll,
{
    state: Arc<State<T, W>>,
    reserved: Option<usize>,
}

impl<T, W> Drop for SinkImpl<T, W>
where
    W: WakeAll,
{
    fn drop(&mut self) {
        self.state.closed.store(true, Ordering::Relaxed);
        self.state.read_waker.wake();
    }
}

impl<T, W> SinkImpl<T, W>
where
    W: WakeAll,
{
    fn poll_reserve(
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

    fn poll_commit(self: Pin<&mut Self>, _: &mut Context, count: usize) -> Poll<Result<(), Error>> {
        if count == 0 {
            return Poll::Ready(Ok(()));
        }
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

struct SingleHead;

struct MultipleHead {
    head: Arc<AtomicUsize>,
    heads: Arc<Mutex<Vec<Arc<AtomicUsize>>>>,
}

trait Head {
    fn get_head<T, W>(&self, state: &State<T, W>) -> usize;

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

struct SourceImpl<T, W, H>
where
    W: WakeAll,
{
    state: Arc<State<T, W>>,
    waker: Arc<AtomicWaker>,
    requested: Option<usize>,
    head: H,
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
    fn poll_request(
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<Status<&[T]>, Error>> {
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

        let slice = if available > 0 {
            let ptr = self.state.offset_ptr(head);
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

    fn poll_consume(
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

/*
pub(crate) fn spmc_buffer<T: Send + Sync + Default + 'static>(
    min_size: usize,
) -> (SinkImpl<T>, SourceImpl<T>) {
    assert!(min_size > 0, "`min_size` must be greater than 0");
    let state = Arc::new(State::new(min_size));

    let (sink_sender, source_receiver) = channel(1);
    let (source_sender, sink_receiver) = channel(1);

    let senders = Arc::new(Mutex::new(vec![sink_sender]));
    let head = Arc::new(AtomicUsize::new(0));
    let heads = Arc::new(Mutex::new(vec![head.clone()]));

    (
        SinkImpl {
            state: state.clone(),
            trigger_receiver: sink_receiver,
            trigger_sender: MultiSender::Multiple(senders.clone()),
        },
        SourceImpl {
            state,
            trigger_receiver: source_receiver,
            trigger_sender: source_sender,
            multi_source: Some(MultiSource {
                head,
                heads,
                senders: Arc::downgrade(&senders),
            }),
        },
    )
}

pub(crate) fn spsc_buffer<T: Send + Sync + Default + 'static>(
    min_size: usize,
) -> (SinkImpl<T>, SourceImpl<T>) {
    assert!(min_size > 0, "`min_size` must be greater than 0");
    let state = Arc::new(State::new(min_size));

    let (sink_sender, source_receiver) = channel(1);
    let (source_sender, sink_receiver) = channel(1);

    (
        SinkImpl {
            state: state.clone(),
            trigger_receiver: sink_receiver,
            trigger_sender: MultiSender::Single(Box::new(sink_sender)),
        },
        SourceImpl {
            state,
            trigger_receiver: source_receiver,
            trigger_sender: source_sender,
            multi_source: None,
        },
    )
}
*/

pub mod spmc {
    use super::*;

    /// Creates a single-producer, multiple-consumer async buffer.
    ///
    /// The buffer can store at least `min_size` elements, but might hold more.
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
    pub struct BufferSink<T: Send + Sync + 'static>(SinkImpl<T, MultipleWaker>);

    /// Read values from the associated `BufferSink`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    #[derive(Clone)]
    pub struct BufferSource<T>(SourceImpl<T, MultipleWaker, MultipleHead>);
}

pub mod spsc {
    use super::*;

    /// Creates a single-producer, single-consumer async buffer.
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

    /// Write values to the associated `BufferSource`s.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    pub struct BufferSink<T: Send + Sync + 'static>(SinkImpl<T, SingleWaker>);

    /// Read values from the associated `BufferSink`.
    ///
    /// Created by the [`buffer`] function.
    ///
    /// [`buffer`]: fn.buffer.html
    pub struct BufferSource<T>(SourceImpl<T, SingleWaker, SingleHead>);
}
