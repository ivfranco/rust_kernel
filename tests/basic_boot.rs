#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![no_main]

use core::panic::PanicInfo;

use rust_kernel::println;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();
    // test_main calls into test_runner which always exits QEMU.
    unreachable!();
}
#[test_case]
fn test_println() {
    println!("test_println output");
}
