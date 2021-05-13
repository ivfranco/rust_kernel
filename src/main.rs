//! A barebone kernel in Rust targeting x86_64, following instructions on [Writing an OS in
//! Rust](https://os.phil-opp.com/), a series of blog posts by Philipp Oppermann.

#![feature(custom_test_frameworks)]
#![test_runner(rust_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;

use alloc::rc::Rc;
use bootloader::{entry_point, BootInfo};
use rust_kernel::{allocator, init, memory};
use rust_kernel::{memory::BootInfoFrameAllocator, println};
use x86_64::VirtAddr;

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
    init();

    #[cfg(test)]
    {
        test_main();
        // `test_main` calls into test_runner which always exits QEMU. A test execution ends here.
    }

    println!("It didn't crash!");

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    // # Safety
    // The physical memory is correctly mapped to the region starting at virtual address
    // phys_mem_offset per bootloader.
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    // # Safety
    // The memory map is valid per bootloader.
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    let heap_value = alloc::boxed::Box::new(42);
    println!("heap_value at {:p}", heap_value);

    let mut vec = alloc::vec::Vec::new();
    vec.extend(0..500);
    println!("vec at {:p}", vec.as_slice());

    let reference_counted = Rc::new(alloc::vec![1, 2, 3]);
    let cloned_reference = reference_counted.clone();
    println!(
        "current reference count is {}",
        Rc::strong_count(&cloned_reference)
    );
    core::mem::drop(reference_counted);
    println!(
        "current reference count is {}",
        Rc::strong_count(&cloned_reference)
    );

    rust_kernel::hlt_loop();
}
