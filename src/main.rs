#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    const VGA_BUFFER: *mut u8 = 0xb8000 as *mut u8;
    const HELLO: &[u8] = b"Hello World!";
    const CYAN: u8 = 0xb;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *VGA_BUFFER.add(i * 2) = byte;
            *VGA_BUFFER.add(i * 2 + 1) = CYAN;
        }
    }

    loop {}
}
