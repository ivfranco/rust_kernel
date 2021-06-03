//! A barebone kernel in Rust targeting x86_64, following instructions on [Writing an OS in
//! Rust](https://os.phil-opp.com/), a series of blog posts by Philipp Oppermann.

#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;

use bootloader::{entry_point, BootInfo};
use rust_kernel::println;
use rust_kernel::task::keyboard;
use rust_kernel::task::Task;
use rust_kernel::{hlt_loop, init, task};

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

fn kernel_main(boot_info: &'static BootInfo) -> ! {
    init(boot_info);

    #[cfg(test)]
    {
        test_main();
        // `test_main` calls into test_runner which always exits QEMU. A test execution ends here.
    }

    println!("It didn't crash!");

    let mut executor = task::executor::Executor::new();
    executor.spawn(Task::new(keyboard::print_keypresses()));
    executor.run();

    hlt_loop();
}
