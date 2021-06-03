//! A non-spinning executor.

use core::task::{Context, Poll, Waker};

use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use crossbeam_queue::ArrayQueue;

use super::{Task, TaskId};

const QUEUE_SIZE: usize = 100;

/// A non-spinning, FIFO executor that makes proper use of wakers.
pub struct Executor {
    tasks: BTreeMap<TaskId, Task>,
    task_queue: Arc<ArrayQueue<TaskId>>,
    waker_cache: BTreeMap<TaskId, Waker>,
}

impl Executor {
    /// Create a new empty [Executor] with the default capacity.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            task_queue: Arc::new(ArrayQueue::new(QUEUE_SIZE)),
            waker_cache: BTreeMap::new(),
        }
    }

    /// Spawn a new task onto the executor.
    pub fn spawn(&mut self, task: Task) {
        let task_id = task.id;
        if self.tasks.insert(task.id, task).is_some() {
            panic!("the same task is spawned twice, should be impossible as spawn() takes ownership of the task");
        }
        self.task_queue.push(task_id).expect("task queue is full");
    }

    /// Kick start the executor, poll all the tasks in FIFO order.
    pub fn run(&mut self) -> ! {
        loop {
            // sleep_if_idle() must also check the task queue because ...
            self.sleep_if_idle();
            self.run_ready_tasks();
            // ... a hardware interrupt may happen right after returning from run_ready_tasks()
        }
    }

    fn sleep_if_idle(&self) {
        use x86_64::instructions::interrupts;

        interrupts::disable();
        if self.task_queue.is_empty() {
            // a hardware interrupt may happen between the condition check and hlt(), interrupts
            // must be disabled in between, otherwise the computer will halt until the next
            // interrupt
            interrupts::enable_and_hlt();
        } else {
            interrupts::enable();
        }
    }

    fn run_ready_tasks(&mut self) {
        let Self {
            tasks,
            task_queue,
            waker_cache,
        } = self;

        while let Some(task_id) = task_queue.pop() {
            let task = match tasks.get_mut(&task_id) {
                Some(task) => task,
                // futures may register the waker spuriously after completion
                None => continue,
            };

            let waker = waker_cache.entry(task_id).or_insert_with(|| {
                let waker = TaskWaker::new(task_id, Arc::clone(task_queue));
                Waker::from(Arc::new(waker))
            });

            let mut context = Context::from_waker(&waker);

            match task.poll(&mut context) {
                Poll::Ready(()) => {
                    tasks.remove(&task_id);
                    waker_cache.remove(&task_id);
                }
                Poll::Pending => (),
            }
        }
    }
}

struct TaskWaker {
    task_id: TaskId,
    task_queue: Arc<ArrayQueue<TaskId>>,
}

impl TaskWaker {
    fn new(task_id: TaskId, task_queue: Arc<ArrayQueue<TaskId>>) -> Self {
        Self {
            task_id,
            task_queue,
        }
    }

    fn wake_task(&self) {
        self.task_queue
            .push(self.task_id)
            .expect("task queue is full");
    }
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_task()
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_task();
    }
}
