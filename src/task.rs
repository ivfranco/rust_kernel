use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

pub mod keyboard;

pub mod simple_executor;

/// An asynchronous task.
pub struct Task {
    /// a pinned, heap allocated, and dynamically dispatched future with no output.
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    /// Create a [Task] from a future with no return value.
    pub fn new(future: impl Future<Output = ()> + 'static) -> Self {
        Self {
            future: Box::pin(future),
        }
    }

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}
