use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
};

pub mod executor;
pub mod keyboard;
pub mod simple_executor;

/// An asynchronous task.
pub struct Task {
    /// A globally unique task id.
    id: TaskId,
    /// a pinned, heap allocated, and dynamically dispatched future with no output.
    future: Pin<Box<dyn Future<Output = ()>>>,
}

impl Task {
    /// Create a [Task] from a future with no return value.
    pub fn new(future: impl Future<Output = ()> + 'static) -> Self {
        Self {
            id: TaskId::new(),
            future: Box::pin(future),
        }
    }

    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TaskId(u64);

impl TaskId {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(0);
        // the order of the operations doesn't matter as long as the ids are unique
        TaskId(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }
}
