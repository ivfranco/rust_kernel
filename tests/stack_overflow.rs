#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::panic::PanicInfo;

use lazy_static::lazy_static;
use rust_kernel::{exit_qemu, serial_print, serial_println, QemuExitCode};
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
    static ref TEST_IDT: InterruptDescriptorTable = {
        // initialize the GDT, per safety requirement of the IDT. Otherwise the methods of
        // [InterruptDescriptorTable] is no longer safe as
        // [rust_kernel::gdt::DOUBLE_FAULT_IST_INDEX] points to uninitialized entry in the IST.
        rust_kernel::gdt::init();

        let mut idt = InterruptDescriptorTable::new();

        // # Safety
        // See [rust_kernel::interrupts::IDT].
        unsafe {
            idt.double_fault
                .set_handler_fn(test_double_fault_handler)
                .set_stack_index(rust_kernel::gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt
    };
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_kernel::test_panic_handler(info);
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    serial_print!("stack_overflow::stack_overflow...\t");

    init_test_idt();

    stack_overflow();

    panic!("Execution continued after stack overflow");
}

fn init_test_idt() {
    TEST_IDT.load();
}

extern "x86-interrupt" fn test_double_fault_handler(
    _stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
}

#[allow(unconditional_recursion)]
fn stack_overflow() {
    stack_overflow();
    // otherwise the compiler may happily apply tail recursion optimization or even remove the
    // entire loop
    volatile::Volatile::new(0).read();
}
