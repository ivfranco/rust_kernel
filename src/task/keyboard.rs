//! Asynchronous keyboard input handling.

use core::{
    pin::Pin,
    task::{Context, Poll},
};

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use futures_util::{task::AtomicWaker, Stream};

use crate::println;

static WAKER: AtomicWaker = AtomicWaker::new();
static SCHANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
const QUEUE_SIZE: usize = 100;

pub(crate) fn add_scancode(scancode: u8) {
    if let Ok(queue) = SCHANCODE_QUEUE.try_get() {
        if queue.push(scancode).is_err() {
            println!("WARNING: scancode full; dropping keyboard input");
        }
    } else {
        println!("WARNING: scancode queue uninitialized");
    }
}

/// A stream of keyboard scancodes produced asynchronously by hardware interrupts.
pub struct ScancodeStream {
    _private: (),
}

impl ScancodeStream {
    /// Create the [ScancodeStream]. Creating more than one [ScancodeStream] this way causes kernel
    /// panic.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        SCHANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(QUEUE_SIZE))
            .expect("ScancodeStream::new should only be called once");

        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let queue = SCHANCODE_QUEUE
            .try_get()
            .expect("SCANCODE_QUEUE not initialized");

        if let Some(scancode) = queue.pop() {
            return Poll::Ready(Some(scancode));
        }

        WAKER.register(&cx.waker());

        // The kernel interrupt handler may have filled the queue after the first check. A second
        // check ensures no keyboard events are lost.
        match queue.pop() {
            Some(scancode) => {
                WAKER.take();
                Poll::Ready(Some(scancode))
            }
            None => Poll::Pending,
        }
    }
}
