//! A single-producer, single-consumer async buffer.

use crate::stream::{Sink, Source};
use async_trait::async_trait;
use slice_deque::Buffer;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use tokio::sync::mpsc::{channel, Receiver, Sender};

/// Creates a single-producer, single-consumer async buffer.
///
/// The buffer can store at least `min_size` elements, but might hold more.
/// # Panics
/// Panics if `min_size` is 0.
pub fn buffer<T: Send + Sync + Default + 'static>(
    min_size: usize,
) -> (BufferSink<T>, BufferSource<T>) {
    assert!(min_size > 0, "`min_size` must be greater than 0");
    let state = Arc::new(State::new(min_size));

    let (sink_sender, source_receiver) = channel(1);
    let (source_sender, sink_receiver) = channel(1);

    (
        BufferSink::<T> {
            state: state.clone(),
            size: 0,
            trigger_receiver: sink_receiver,
            trigger_sender: sink_sender,
        },
        BufferSource::<T> {
            state,
            size: 0,
            trigger_receiver: source_receiver,
            trigger_sender: source_sender,
        },
    )
}

struct State<T> {
    buffer: Buffer<T>,
    size: usize,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl<T: Default> State<T> {
    fn new(min_size: usize) -> Self {
        // Initialize the buffer memory
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

/// Write values to the associated `BufferSource`.
///
/// Created by the [`buffer`] function.
///
/// [`buffer`]: fn.buffer.html
pub struct BufferSink<T: Send + Sync + 'static> {
    state: Arc<State<T>>,
    size: usize,
    trigger_receiver: Receiver<()>,
    trigger_sender: Sender<()>,
}

#[async_trait]
impl<T: Send + Sync + 'static> Sink for BufferSink<T> {
    type Item = T;

    fn len(&self) -> usize {
        self.size
    }

    async fn advance(&mut self, advance: usize, size: usize) -> Option<&mut [T]> {
        assert!(
            advance <= self.size,
            "cannot advance past end of write buffer"
        );
        assert!(
            size <= (self.state.size - 1),
            "cannot request write buffer larger than total buffer"
        );
        self.state.advance_tail(advance);

        // Notify source that data is available
        // If the source is gone, there's no point sinking more data
        if let Err(e) = self.trigger_sender.try_send(()) {
            if e.is_closed() {
                self.size = 0;
                return None;
            }
        }

        // Wait for enough room to advance
        while self.state.writable_len() < size {
            if self.trigger_receiver.recv().await.is_none() {
                self.size = 0;
                return None;
            }
        }
        self.size = size;

        // Return the mutable slice
        let ptr = self.state.tail_ptr();
        Some(unsafe { std::slice::from_raw_parts_mut(ptr, self.size) })
    }
}

/// Read values from the associated `BufferSink`.
///
/// Created by the [`buffer`] function.
///
/// [`buffer`]: fn.buffer.html
pub struct BufferSource<T> {
    state: Arc<State<T>>,
    size: usize,
    trigger_receiver: Receiver<()>,
    trigger_sender: Sender<()>,
}

#[async_trait]
impl<T: Send + Sync + 'static> Source for BufferSource<T> {
    type Item = T;

    fn len(&self) -> usize {
        self.size
    }

    async fn advance(&mut self, advance: usize, size: usize) -> Option<&[T]> {
        assert!(
            advance <= self.size,
            "cannot advance past end of read buffer"
        );
        assert!(
            size <= (self.state.size - 1),
            "cannot request read buffer larger than total buffer"
        );
        self.state.advance_head(advance);

        // Notify sink that space is available
        // If the sink is gone, we continue because there might still be data in the buffer
        let _ = self.trigger_sender.try_send(());

        // Wait for enough data to read
        while self.state.readable_len() < size {
            if self.trigger_receiver.recv().await.is_none() {
                if self.state.readable_len() == 0 {
                    self.size = 0;
                    return None;
                } else {
                    break;
                }
            }
        }
        self.size = std::cmp::min(self.state.readable_len(), size);

        // Return the slice
        let ptr = self.state.head_ptr();
        Some(unsafe { std::slice::from_raw_parts(ptr, self.size) })
    }
}
