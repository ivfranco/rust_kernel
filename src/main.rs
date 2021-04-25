//! A barebone kernel in Rust targeting x86_64, following instructions on [Writing an OS in
//! Rust](https://os.phil-opp.com/), a series of blog posts by Philipp Oppermann.

#![no_std]
#![no_main]
#![deny(missing_docs)]

mod vga_buffer;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/// Entry point of the kernel expected by lld.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    for r in 0..30 {
        println!("Hello World! {}", r);
    }

    loop {}
}
