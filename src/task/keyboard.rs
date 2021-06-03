//! Asynchronous keyboard input handling.

use core::{
    pin::Pin,
    task::{Context, Poll},
};

use conquer_once::spin::OnceCell;
use crossbeam_queue::ArrayQueue;
use futures_util::{task::AtomicWaker, Stream, StreamExt};
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

use crate::{print, println};

static WAKER: AtomicWaker = AtomicWaker::new();
static SCANCODE_QUEUE: OnceCell<ArrayQueue<u8>> = OnceCell::uninit();
const QUEUE_SIZE: usize = 100;

pub(crate) fn add_scancode(scancode: u8) {
    let queue = match SCANCODE_QUEUE.try_get() {
        Ok(queue) => queue,
        Err(_) => {
            println!("WARNING: scancode queue uninitialized");
            return;
        }
    };

    if queue.push(scancode).is_err() {
        println!("WARNING: scancode full; dropping keyboard input");
        return;
    }

    // inform the executor about the keyboard event if the waker is registered
    WAKER.wake();
}

/// print key events
pub async fn print_keypresses() {
    let mut scancodes = ScancodeStream::new();
    let mut keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);

    while let Some(scancode) = scancodes.next().await {
        // Processing a byte read from the PS/2 data port may not always be successful: the scancode
        // may be invalid, the scancode may lead to an impossible state assuming the keyboard
        // layout, the scancode may be corrupted by transmission, etc. Processing a byte may also
        // not return a key event, e.g. the escape byte before extended keycode.
        if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
            // Press and release are two separate events in IBM XT. Here only key presses are mapped
            // to characters.
            if let Some(key) = keyboard.process_keyevent(key_event) {
                match key {
                    DecodedKey::RawKey(key) => print!("{:?}", key),
                    DecodedKey::Unicode(code) => print!("{}", code),
                }
            }
        }
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
        SCANCODE_QUEUE
            .try_init_once(|| ArrayQueue::new(QUEUE_SIZE))
            .expect("ScancodeStream::new should only be called once");

        ScancodeStream { _private: () }
    }
}

impl Stream for ScancodeStream {
    type Item = u8;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let queue = SCANCODE_QUEUE
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
