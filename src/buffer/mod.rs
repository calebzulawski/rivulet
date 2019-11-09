//! Buffers for temporarily caching data.

pub mod spmc;
pub mod spsc;

use slice_deque::Buffer;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex, Weak,
};
use tokio::sync::mpsc::{channel, Receiver, Sender};

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

enum MultiSender {
    Single(Box<Sender<()>>),
    Multiple(Arc<Mutex<Vec<Sender<()>>>>),
}

struct SinkImpl<T> {
    state: Arc<State<T>>,
    trigger_receiver: Receiver<()>,
    trigger_sender: MultiSender,
    size: usize,
}

impl<T> SinkImpl<T> {
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
        self.state.advance_value(advance, &self.state.tail);

        // Notify source(s) that data is available
        // If the source is gone, there's no point sinking more data
        let connected = match &mut self.trigger_sender {
            MultiSender::Single(sender) => {
                if let Err(e) = sender.try_send(()) {
                    !e.is_closed()
                } else {
                    true
                }
            }
            MultiSender::Multiple(senders) => {
                let mut senders = senders.lock().unwrap();
                for idx in (0..senders.len()).rev() {
                    if let Err(e) = senders[idx].try_send(()) {
                        if e.is_closed() {
                            senders.remove(idx);
                        }
                    }
                }
                !senders.is_empty()
            }
        };
        if !connected {
            self.size = 0;
            return None;
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

struct MultiSource {
    head: Arc<AtomicUsize>,
    heads: Arc<Mutex<Vec<Arc<AtomicUsize>>>>,
    senders: Weak<Mutex<Vec<Sender<()>>>>,
}

struct SourceImpl<T> {
    state: Arc<State<T>>,
    size: usize,
    trigger_receiver: Receiver<()>,
    trigger_sender: Sender<()>,
    multi_source: Option<MultiSource>,
}

impl<T> Drop for SourceImpl<T> {
    fn drop(&mut self) {
        // If there are multiple sources, remove this source from the head list
        if let Some(multi_source) = &self.multi_source {
            multi_source
                .heads
                .lock()
                .unwrap()
                .retain(|x| !Arc::ptr_eq(x, &multi_source.head));
        }
    }
}

impl<T> Clone for SourceImpl<T> {
    fn clone(&self) -> Self {
        let multi_source = self.multi_source.as_ref().unwrap();

        // Copy the head value for the new source
        let head = Arc::new(AtomicUsize::new(multi_source.head.load(Ordering::Acquire)));

        // Add the new head
        multi_source.heads.lock().unwrap().push(head.clone());

        // Create the new trigger channel
        let (sender, receiver) = channel(1);

        // Add the trigger sender to the source
        if let Some(senders) = multi_source.senders.upgrade() {
            senders.lock().unwrap().push(sender);
        }

        Self {
            state: self.state.clone(),
            size: self.size,
            trigger_receiver: receiver,
            trigger_sender: self.trigger_sender.clone(),
            multi_source: Some(MultiSource {
                head,
                heads: multi_source.heads.clone(),
                senders: multi_source.senders.clone(),
            }),
        }
    }
}

impl<T> SourceImpl<T> {
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

        let head = if let Some(multi_source) = &self.multi_source {
            // Advance just this head
            self.state.advance_value(advance, &multi_source.head);

            // Find the earliest head in the list
            let heads = multi_source.heads.lock().unwrap();
            assert!(!heads.is_empty());
            let current_head = self.state.head.load(Ordering::Acquire);
            let mut earliest_head = std::usize::MAX;
            let mut smallest_distance = std::usize::MAX;
            for head in heads.iter() {
                let this_head = head.load(Ordering::Acquire);
                let this_distance = self.state.distance(current_head, this_head);
                if this_distance < smallest_distance {
                    earliest_head = this_head;
                    smallest_distance = this_distance;
                }
            }

            // Discard up to the earliest head
            self.state.head.store(earliest_head, Ordering::Release);
            multi_source.head.load(Ordering::Acquire)
        } else {
            // Directly update the head
            self.state.advance_value(advance, &self.state.head);
            self.state.head.load(Ordering::Acquire)
        };

        // Notify sink that space is available
        // If the sink is gone, we continue because there might still be data in the buffer
        let _ = self.trigger_sender.try_send(());

        // Wait for enough data to read
        while self
            .state
            .distance(head, self.state.tail.load(Ordering::Relaxed))
            < size
        {
            if self.trigger_receiver.recv().await.is_none() {
                break;
            }
        }
        self.size = std::cmp::min(
            self.state
                .distance(head, self.state.tail.load(Ordering::Relaxed)),
            size,
        );

        // Return the slice
        if self.size == 0 {
            None
        } else {
            let ptr = self.state.offset_ptr(head);
            Some(unsafe { std::slice::from_raw_parts(ptr, self.size) })
        }
    }
}

fn spmc_buffer<T: Send + Sync + Default + 'static>(
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
            size: 0,
            trigger_receiver: sink_receiver,
            trigger_sender: MultiSender::Multiple(senders.clone()),
        },
        SourceImpl {
            state,
            size: 0,
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

fn spsc_buffer<T: Send + Sync + Default + 'static>(
    min_size: usize,
) -> (SinkImpl<T>, SourceImpl<T>) {
    assert!(min_size > 0, "`min_size` must be greater than 0");
    let state = Arc::new(State::new(min_size));

    let (sink_sender, source_receiver) = channel(1);
    let (source_sender, sink_receiver) = channel(1);

    (
        SinkImpl {
            state: state.clone(),
            size: 0,
            trigger_receiver: sink_receiver,
            trigger_sender: MultiSender::Single(Box::new(sink_sender)),
        },
        SourceImpl {
            state,
            size: 0,
            trigger_receiver: source_receiver,
            trigger_sender: source_sender,
            multi_source: None,
        },
    )
}
