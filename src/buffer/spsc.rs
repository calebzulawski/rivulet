use crate::stream;
use async_trait::async_trait;
use slice_deque::Buffer;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};
use tokio::sync::mpsc::{UnboundedSender, UnboundedReceiver};

struct State<T> {
    buffer: Buffer<T>,
    head: AtomicUsize,
    tail: AtomicUsize,
}

impl<T> State<T> {
    unsafe fn get_write_slice(&self, size: usize) -> &mut [T] {
        std::slice::from_raw_parts_mut(self.buffer.ptr().add(self.tail.load(Ordering::Relaxed)), size)
    }

    unsafe fn get_read_slice(&self, size: usize) -> &[T] {
        std::slice::from_raw_parts(self.buffer.ptr().add(self.head.load(Ordering::Relaxed)), size)
    }

    fn advance_tail(&self, advance: usize) {
        let new_tail = (self.tail.load(Ordering::Relaxed) + advance) % self.buffer.len();
        self.tail.store(new_tail, Ordering::Relaxed);
    }

    fn advance_head(&self, advance: usize) {
        let new_head = (self.head.load(Ordering::Relaxed) + advance) % self.buffer.len();
        self.head.store(new_head, Ordering::Relaxed);
    }

    fn writable_len(&self) -> usize {
        (self.head.load(Ordering::Relaxed) - self.tail.load(Ordering::Relaxed) + self.buffer.len()) % self.buffer.len()
    }

    fn readable_len(&self) -> usize {
        (self.tail.load(Ordering::Relaxed) - self.head.load(Ordering::Relaxed) + self.buffer.len()) % self.buffer.len()
    }
}

pub struct Sink<T> {
    state: Arc<State<T>>,
    size: usize,
    trigger_receiver: UnboundedReceiver<()>,
    trigger_sender: UnboundedSender<()>,
}

#[async_trait]
impl<T> stream::Sink<T> for Sink<T> where T: Send + Sync + 'static {
    fn as_mut_slice(&mut self) -> &mut [T] {
        let size = self.len();
        unsafe { self.state.get_write_slice(size) }
    }

    fn len(&self) -> usize {
        self.size
    }

    async fn advance(mut self, advance: usize, size: usize) -> Option<Self> {
        assert!(advance <= self.len(), "cannot advance past end of write buffer");
        assert!(size <= self.state.buffer.len(), "cannot request write buffer larger than total buffer");
        self.state.advance_tail(advance);
        if self.trigger_sender.try_send(()).is_err() {
            return None
        }
        while self.state.writable_len() < size {
            self.trigger_receiver.recv().await?;
        }
        Some(Sink::<T> {size, ..self})
    }
}

pub struct Source<T> {
    state: Arc<State<T>>,
    size: usize,
    trigger_receiver: UnboundedReceiver<()>,
    trigger_sender: UnboundedSender<()>,
}

#[async_trait]
impl<T> stream::Source<T> for Source<T> where T: Send + Sync + 'static {
    fn as_slice(&self) -> & [T] {
        let size = self.len();
        unsafe { self.state.get_read_slice(size) }
    }

    fn len(&self) -> usize {
        self.size
    }

    async fn advance(mut self, advance: usize, size: usize) -> Option<Self> {
        assert!(advance <= self.len(), "cannot advance past end of read buffer");
        assert!(size <= self.state.buffer.len(), "cannot request read buffer larger than total buffer");
        self.state.advance_head(advance);
        if self.trigger_sender.try_send(()).is_err() {
            return None
        }
        while self.state.readable_len() < size {
            self.trigger_receiver.recv().await?;
        }
        Some(Source::<T> {size, ..self})
    }
}
