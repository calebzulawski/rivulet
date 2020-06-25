use crate::stream::{Error, Sink, Source};
use pin_project::pin_project;
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

macro_rules! method {
    { $func:ident => $type:ident } => {
        fn $func<'a>(&'a mut self, count: usize) -> $type<'a, Self>
        where
            Self: Sized + Unpin,
        {
            $type { handle: self, count }
        }
    }
}

pub trait SinkExt: Sink {
    method! { reserve => Reserve }
    method! { commit => Commit }
}

impl<S: Sink> SinkExt for S {}

pub trait SourceExt: Source {
    method! { request => Request }
    method! { consume => Consume }
}

impl<S: Source> SourceExt for S {}

macro_rules! future {
    { $trait:ident => $type:ident => $poll:ident } => {
        #[pin_project]
        pub struct $type<'a, T> {
            #[pin]
            handle: &'a mut T,
            count: usize,
        }

        impl<'a, T> Future for $type<'a, T>
        where
            T: $trait + Unpin,
        {
            type Output = Result<(), Error>;

            fn poll(self: Pin<&mut Self>, cx: &mut Context) -> Poll<Self::Output> {
                let count = self.count;
                let pinned = self.project();
                pinned.handle.$poll(cx, count)
            }
        }
    }
}

future! { Sink => Reserve => poll_reserve }
future! { Sink => Commit => poll_commit }
future! { Source => Request => poll_request }
future! { Source => Consume => poll_consume }
