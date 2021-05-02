//! A barebone kernel in Rust targeting x86_64, following instructions on [Writing an OS in
//! Rust](https://os.phil-opp.com/), a series of blog posts by Philipp Oppermann.

#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![no_main]
#![deny(missing_docs)]

use core::panic::PanicInfo;

#[cfg(not(test))]
use rust_kernel::println;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {}
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info);
}

/// Entry point of the kernel expected by lld.
#[no_mangle]
pub extern "C" fn _start() -> ! {
    #[cfg(test)]
    {
        test_main();
        // test_main calls into test_runner which always exits QEMU.
        unreachable!();
    }

    #[cfg(not(test))]
    {
        for r in 0..30 {
            println!("Hello World! {}", r);
        }

        panic!();
    }
}
