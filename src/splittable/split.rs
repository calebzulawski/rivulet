use super::{SplittableSource, SplittableSourceMut};
use futures::task::AtomicWaker;
use pin_project::pin_project;
use std::{
    convert::TryInto,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, RwLock,
    },
    task::{Context, Poll},
};

struct Reader {
    waker: AtomicWaker,
    head: AtomicU64,
}

pub(crate) struct SplitSignal(RwLock<Vec<Arc<Reader>>>);

impl SplitSignal {
    /// Create a new signal with one reader
    pub(crate) fn new() -> Self {
        let reader = Reader {
            waker: AtomicWaker::new(),
            head: AtomicU64::new(0),
        };
        Self(RwLock::new(vec![Arc::new(reader)]))
    }

    /// Wake all readers
    pub(crate) fn wake(&self) {
        let lock = self.0.read().expect("another thread panicked");
        for reader in lock.iter() {
            reader.waker.wake();
        }
    }

    /// Copy the specified reader
    fn insert(&self, reader: &Arc<Reader>) -> Arc<Reader> {
        let reader = Arc::new(Reader {
            waker: AtomicWaker::new(),
            head: AtomicU64::new(reader.head.load(Ordering::Relaxed)),
        });
        let mut lock = self.0.write().expect("another thread panicked");
        lock.push(reader.clone());
        reader
    }

    /// Removes the specified reader
    fn remove(&self, reader: &Arc<Reader>) -> bool {
        let mut lock = self.0.write().expect("another thread panicked");
        lock.retain(|test_reader| !Arc::ptr_eq(test_reader, reader));
        lock.is_empty()
    }
}

#[pin_project]
pub struct Split<T>
where
    T: SplittableSource,
{
    source: Arc<T>,
    this_reader: Arc<Reader>,
    signal: Arc<SplitSignal>,
    head: u64,
    len: usize,
}

impl<T> Split<T>
where
    T: SplittableSource,
{
    pub(crate) fn new(signal: Arc<SplitSignal>, source: T) -> Self {
        let this_reader = signal.0.read().expect("another thread panicked")[0].clone();
        Self {
            source: Arc::new(source),
            this_reader,
            signal,
            head: 0,
            len: 0,
        }
    }
}

impl<T> crate::View for Split<T>
where
    T: SplittableSource + Unpin,
{
    type Item = T::Item;
    type Error = T::Error;

    fn view(&self) -> &[Self::Item] {
        // we have unique ownership of the source, so this doesn't overlap with any other views
        unsafe { self.source.view(self.head, self.len) }
    }

    fn poll_grant(
        self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::Error>> {
        let pinned = self.project();
        match Pin::new(pinned.source.as_ref()).poll_available(
            cx,
            |waker| pinned.this_reader.waker.register(waker),
            *pinned.head,
            count,
        ) {
            Poll::Ready(Ok(len)) => {
                *pinned.len = len;
                Poll::Ready(Ok(()))
            }
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }

    fn release(&mut self, count: usize) {
        self.len -= count;
        let count: u64 = count.try_into().unwrap();
        self.head += count;
        self.this_reader.head.store(self.head, Ordering::Relaxed);
        self.source.as_ref().compare_set_head(self.head);
    }
}

impl<T> crate::ViewMut for Split<T>
where
    T: SplittableSourceMut + Unpin,
{
    fn view_mut(&mut self) -> &mut [Self::Item] {
        // we have unique ownership of the source, so this doesn't overlap with any other views
        unsafe { self.source.view_mut(self.head, self.len) }
    }
}

impl<T> crate::Source for Split<T> where T: SplittableSource + Unpin {}
