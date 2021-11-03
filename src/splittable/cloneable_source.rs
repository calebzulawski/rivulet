use super::SplittableSource;
use futures::task::AtomicWaker;
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

pub(crate) struct CloneableSignal(RwLock<Vec<Arc<Reader>>>);

impl CloneableSignal {
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
    fn remove(&self, reader: &Arc<Reader>) {
        let mut lock = self.0.write().expect("another thread panicked");
        lock.retain(|test_reader| !Arc::ptr_eq(test_reader, reader));
    }
}

/// A source returned by
/// [`Splittable::into_cloneable_source`](`super::Splittable::into_cloneable_source`).
///
/// This source may be cloned to be used with other readers.  The cloned source is initialized with
/// the same view of the stream.
pub struct Cloneable<T>
where
    T: SplittableSource,
{
    source: Arc<T>,
    this_reader: Arc<Reader>,
    signal: Arc<CloneableSignal>,
    head: u64,
    len: usize,
}

impl<T> Cloneable<T>
where
    T: SplittableSource,
{
    pub(crate) fn new(signal: Arc<CloneableSignal>, source: T) -> Self {
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

impl<T> Clone for Cloneable<T>
where
    T: SplittableSource,
{
    fn clone(&self) -> Self {
        let this_reader = self.signal.insert(&self.this_reader);
        Self {
            source: self.source.clone(),
            this_reader,
            signal: self.signal.clone(),
            head: self.head,
            len: self.len,
        }
    }
}

impl<T> Drop for Cloneable<T>
where
    T: SplittableSource,
{
    fn drop(&mut self) {
        self.signal.remove(&self.this_reader);
    }
}

impl<T> crate::View for Cloneable<T>
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
        mut self: Pin<&mut Self>,
        cx: &mut Context,
        count: usize,
    ) -> Poll<Result<(), Self::Error>> {
        match Pin::new(self.source.as_ref()).poll_available(
            cx,
            |waker| self.this_reader.waker.register(waker),
            self.head,
            count,
        ) {
            Poll::Ready(Ok(len)) => {
                self.len = len;
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

impl<T> crate::Source for Cloneable<T> where T: SplittableSource + Unpin {}
