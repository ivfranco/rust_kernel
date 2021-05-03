use crate::println;

use crate::gdt;
use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // all interruption handlers should use the `x86-interrupt` FFI, the normal calling
        // convention assuming caller / callee-saved registers and CPU flag consistency is not
        // suitable as CPU interruption may be triggered by any instruction and change the state of
        // CPU flags.
        idt.breakpoint.set_handler_fn(breakpoint_handler);

        // Switch to the separate stack space on double fault.
        //
        // # Safety
        // [gdt::DOUBLE_FAULT_IST_INDEX] is a valid Interrupt Stack Table index to an entry properly
        // initialized on the initialization of TSS, the stack index is not used on any other
        // interrrupts. [init_idt], the only function accessing this global variable is unsafe.
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler).set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt
    };
}

/// Initialize the Interrupt Description Table. Currently the following handlers are defined:
/// - breakpoint
/// - double fault
///
/// # Safety
/// This function is unsafe because the IDT refers to an entry in the Interrupt Stack Table which
/// must be initialized by the GDT. Calling this function before [gdt::init] refers to uninitialized
/// memory.
pub unsafe fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

#[cfg(test)]
mod tests {
    #[test_case]
    fn breakpoint_exception_handled() {
        x86_64::instructions::interrupts::int3();
        // if the test function returns, kernel is not terminated
    }

    // #[test_case]
    // fn page_fault_exception_handled() {
    //     unsafe {
    //         *(0xdeadbeef as *mut u64) = 42;
    //     }
    //     // if the test function returns, kernel is not terminated
    // }
}
