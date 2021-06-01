//! A deliberately very basic executor.

use alloc::collections::VecDeque;
use core::{
    ptr,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};

use super::Task;

/// A very basic executor based on a FIFO queue.
#[derive(Default)]
pub struct SimpleExecutor {
    task_queue: VecDeque<Task>,
}

impl SimpleExecutor {
    /// Create an new empty [SimpleExecutor].
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a new task onto the executor.
    pub fn spawn(&mut self, task: Task) {
        self.task_queue.push_back(task)
    }

    /// Kick start the executor, busily poll all the tasks in Round-Robin fashion.
    pub fn run(&mut self) {
        while let Some(mut task) = self.task_queue.pop_front() {
            let waker = dummy_waker();
            let mut context = Context::from_waker(&waker);
            match task.poll(&mut context) {
                Poll::Ready(_) => {}
                Poll::Pending => self.task_queue.push_back(task),
            }
        }
    }
}

fn dummy_raw_waker() -> RawWaker {
    fn no_op(_: *const ()) {}
    fn clone(_: *const ()) -> RawWaker {
        dummy_raw_waker()
    }

    let vtable = &RawWakerVTable::new(clone, no_op, no_op, no_op);
    RawWaker::new(ptr::null(), vtable)
}

fn dummy_waker() -> Waker {
    unsafe { Waker::from_raw(dummy_raw_waker()) }
}
