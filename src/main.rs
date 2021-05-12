//! A barebone kernel in Rust targeting x86_64, following instructions on [Writing an OS in
//! Rust](https://os.phil-opp.com/), a series of blog posts by Philipp Oppermann.

#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

use bootloader::{entry_point, BootInfo};
use rust_kernel::init;
#[cfg(not(test))]
use rust_kernel::println;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    rust_kernel::hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info);
}

// The type signuature of the kernel entry point cannot be type checked in Rust, the compiler
// doesn't know _start is the entry point nor what form it should take. the macro entry_point!
// enforces a correct type to the kernel entry point function.
entry_point!(kernel_main);

fn kernel_main(_boot_info: &'static BootInfo) -> ! {
    init();

    #[cfg(test)]
    {
        test_main();
        // test_main calls into test_runner which always exits QEMU.
        unreachable!();
    }

    #[cfg(not(test))]
    {
        println!("It didn't crash!");
        rust_kernel::hlt_loop();
    }
}
