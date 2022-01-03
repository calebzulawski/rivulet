use crate::splittable::{Splittable, SplittableImpl, SplittableImplMut, SplittableMut};
use once_cell::sync::OnceCell;
use std::{
    convert::TryInto,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    task::{Context, Poll, Waker},
};

pub(super) fn make_sequence<T>(splittable: T) -> (First<T>, Second<T>)
where
    T: Splittable,
{
    let shared = Arc::new(Shared {
        splittable,
        head: AtomicU64::new(0),
        closed: AtomicBool::new(false),
        waker: Mutex::new(None),
    });

    (
        First {
            shared: shared.clone(),
            waker: OnceCell::new(),
        },
        Second { shared },
    )
}

struct Shared<T>
where
    T: Splittable,
{
    splittable: T,
    head: AtomicU64,
    closed: AtomicBool,
    waker: Mutex<Option<Box<dyn Fn() + Send + Sync + 'static>>>,
}

/// The first `Splittable` produced by [`sequence`](`crate::Splittable::sequence`).
pub struct First<T>
where
    T: Splittable,
{
    shared: Arc<Shared<T>>,
    waker: OnceCell<Box<dyn Fn() + Send + Sync + 'static>>,
}

impl<T> Drop for First<T>
where
    T: Splittable,
{
    fn drop(&mut self) {
        self.shared.closed.store(true, Ordering::Relaxed);
        self.wake_second()
    }
}

impl<T> First<T>
where
    T: Splittable,
{
    fn wake_second(&self) {
        if let Ok(waker) = self.waker.get_or_try_init(|| {
            let mut lock = self.shared.waker.lock().expect("another thread panicked");
            lock.take().ok_or(())
        }) {
            waker()
        }
    }
}

unsafe impl<T> SplittableImpl for First<T>
where
    T: Splittable,
{
    type Item = T::Item;
    type Error = T::Error;

    unsafe fn set_reader_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        self.shared.splittable.set_reader_waker(waker);
    }

    unsafe fn set_head(&self, index: u64) {
        if self.shared.closed.load(Ordering::Relaxed) {
            // This may overlap with a drop of `Second`, so always use `compare_set_head`.
            self.shared.splittable.compare_set_head(index);
        } else {
            self.shared.head.store(index, Ordering::Relaxed);
            self.wake_second();
        }
    }

    unsafe fn compare_set_head(&self, index: u64) {
        if self.shared.closed.load(Ordering::Relaxed) {
            self.shared.splittable.compare_set_head(index);
        } else {
            // only set the head if it's greater than the current head
            let mut current = self.shared.head.load(Ordering::Relaxed);
            if index > current {
                while let Err(previous) = self.shared.head.compare_exchange_weak(
                    current,
                    index,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                ) {
                    if index > previous {
                        current = previous
                    } else {
                        break;
                    }
                }
            }
            self.wake_second()
        }
    }

    fn poll_available(
        self: Pin<&Self>,
        cx: &mut Context,
        register_wakeup: impl Fn(&Waker),
        index: u64,
        len: usize,
    ) -> Poll<Result<usize, Self::Error>> {
        Pin::new(&self.shared.splittable).poll_available(cx, register_wakeup, index, len)
    }

    unsafe fn view(&self, index: u64, len: usize) -> &[Self::Item] {
        self.shared.splittable.view(index, len)
    }
}

unsafe impl<T> SplittableImplMut for First<T>
where
    T: SplittableMut,
{
    unsafe fn view_mut(&self, index: u64, len: usize) -> &mut [Self::Item] {
        self.shared.splittable.view_mut(index, len)
    }
}

/// The second `Splittable` produced by [`sequence`](`crate::Splittable::sequence`).
pub struct Second<T>
where
    T: Splittable,
{
    shared: Arc<Shared<T>>,
}

impl<T> Drop for Second<T>
where
    T: Splittable,
{
    fn drop(&mut self) {
        self.shared.closed.store(true, Ordering::Relaxed);

        // Safety: this view is done with `head` so we can drop up to it.
        // We must use `compare_set_head` since this may overlap with an advance on `First` that
        // happens after `closed` is set but before setting the head occurs.
        unsafe {
            self.shared
                .splittable
                .compare_set_head(self.shared.head.load(Ordering::Relaxed));
        }
    }
}

impl<T> Second<T>
where
    T: Splittable,
{
    fn readable_len(&self, start: u64) -> usize {
        (self.shared.head.load(Ordering::Relaxed) - start)
            .try_into()
            .unwrap()
    }
}

unsafe impl<T> SplittableImpl for Second<T>
where
    T: Splittable,
{
    type Item = T::Item;
    type Error = T::Error;

    unsafe fn set_reader_waker(&self, waker: impl Fn() + Send + Sync + 'static) {
        let mut lock = self.shared.waker.lock().expect("another thread panicked!");
        *lock = Some(Box::new(waker));
    }

    unsafe fn set_head(&self, index: u64) {
        self.shared.splittable.set_head(index);
    }

    unsafe fn compare_set_head(&self, index: u64) {
        self.shared.splittable.compare_set_head(index);
    }

    fn poll_available(
        self: Pin<&Self>,
        cx: &mut Context,
        register_wakeup: impl Fn(&Waker),
        index: u64,
        len: usize,
    ) -> Poll<Result<usize, Self::Error>> {
        // Perform double-checking on the amount of available data
        // The first check is efficient, but may spuriously fail.
        // The second check occurs after the `acquire` produced by registering the waker.
        let available = self.readable_len(index);
        if available >= len {
            Poll::Ready(Ok(available))
        } else {
            register_wakeup(cx.waker());
            let available = self.readable_len(index);
            if available >= len || self.shared.closed.load(Ordering::Relaxed) {
                Poll::Ready(Ok(available))
            } else {
                Poll::Pending
            }
        }
    }

    unsafe fn view(&self, index: u64, len: usize) -> &[Self::Item] {
        self.shared.splittable.view(index, len)
    }
}

unsafe impl<T> SplittableImplMut for Second<T>
where
    T: SplittableMut,
{
    unsafe fn view_mut(&self, index: u64, len: usize) -> &mut [Self::Item] {
        self.shared.splittable.view_mut(index, len)
    }
}
