//! A barebone kernel in Rust targeting x86_64, following instructions on [Writing an OS in
//! Rust](https://os.phil-opp.com/), a series of blog posts by Philipp Oppermann.

#![feature(custom_test_frameworks)]
#![feature(alloc_error_handler)]
#![feature(abi_x86_interrupt)]
#![feature(const_mut_refs)]
#![cfg_attr(test, no_main)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![no_std]
#![deny(missing_docs)]

extern crate alloc;

/// A safe global interface to print text to stdout of QEMU process in form of print macros.
pub mod serial;

/// A safe global interface to the VGA text buffer in form of print macros.
pub mod vga_buffer;

/// Definition and initialization of interruption handlers.
pub mod interrupts;

/// Definition and initialization of the Global Descriptor Table.
pub mod gdt;

/// Functions to manage the page tables, inspect and modify its content, also translate virtual
/// addresses to physical addresses.
pub mod memory;

pub(crate) mod locked;

/// A global allocator for the kernel.
pub mod allocator;

/// Bare minimum code to bootstrap asynchronous tasks as required by Rust standard library.
pub mod task;

#[cfg(test)]
use bootloader::entry_point;
use bootloader::BootInfo;
use x86_64::VirtAddr;

use core::panic::PanicInfo;

use memory::BootInfoFrameAllocator;

/// Port number of isa-debug-exit as defined in package.metadata.bootimage.test-args in Cargo.toml.
const ISA_DEBUG_EXIT_PORT: u16 = 0xf4;

#[alloc_error_handler]
fn alloc_error_handler(layout: core::alloc::Layout) -> ! {
    panic!("allocation error: {:?}", layout)
}

/// Exit code feed to the isa-debug-exit device of QEMU.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    /// Exit code on successful test runs. Maps to an exit status 0x21 in host system.
    Success = 0x10,
    /// Exit code on failed test runs. Maps to an exit status 0x23 in host system.
    Failed = 0x11,
}

/// Write the supplied exit code to the QEMU isa-debug-exit device. The QEMU process will exit (in
/// the host system) with status (code << 1) | 1.
pub fn exit_qemu(exit_code: QemuExitCode) -> ! {
    use x86_64::instructions::port::Port;

    // using port number and data size specified in package.metadata.bootimage.test-args in
    // Cargo.toml.
    //
    // # Safety
    // isa-debug-exit has no memory side effects, even if it had it's not likely to cause UB:
    // successful write to the port immediately terminates the QEMU process.
    unsafe {
        let mut port = Port::<u32>::new(ISA_DEBUG_EXIT_PORT);
        port.write(exit_code as u32);
    }

    // Unreachable: QEMU should be terminated by write to isa-debug-exit. Loop in case QEMU is not
    // immediately shut down.
    hlt_loop();
}

/// Initialize the following components of the kernel:
/// - interruption handlers
pub fn init(boot_info: &'static BootInfo) {
    gdt::init();
    // # Safety
    // GDT is initialized before this call.
    unsafe {
        interrupts::init_idt();
    }
    interrupts::init_pics();

    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    // # Safety
    // The physical memory is correctly mapped to the region starting at virtual address
    // phys_mem_offset per bootloader.
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    // # Safety
    // The memory map is valid per bootloader.
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");
}

/// Put the CPU in a hlt loop, allow the CPU to enter a sleep state until an interrupt arrives and
/// after the interrupt handler returned.
pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}

#[cfg(test)]
entry_point!(test_kernel_main);

#[cfg(test)]
fn test_kernel_main(boot_info: &'static BootInfo) -> ! {
    init(boot_info);
    test_main();
    // test_main calls into test_runner which always exits QEMU.
    unreachable!();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

/// The test panic handler. Output panic info to both VGA text buffer in QEMU and host system then
/// terminate QEMU process.
pub fn test_panic_handler(info: &PanicInfo) -> ! {
    println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
}

/// The sequential test runner. Currently global states are not reset between tests.
pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }

    exit_qemu(QemuExitCode::Success);
}

/// A helper trait that prints test results to the host system.
pub trait Testable {
    /// Run the test, print test name and result to the host system.
    fn run(&self);
}

impl<T: Fn()> Testable for T {
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

#[cfg(test)]
pub mod tests {
    #[test_case]
    fn trivial_assertion() {
        assert_eq!(1 + 1, 2)
    }
}
