use crate::{hlt_loop, print, println};

use crate::gdt;
use lazy_static::lazy_static;
use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
use pic8259_simple::ChainedPics;
use spin::Mutex;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

/// Offset of the first PIC (Programmable Interrupt Controller).
///
/// The first 32 slots of the Interrupt Descriptor Table is occupied by CPU exceptions hence must be
/// avoided. Each PIC occupies 8 slots starting from its offset.
pub const PIC_1_OFFSET: u8 = 32;
/// Offset of the second PIC (Programmable Interrupt Controller).
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

static PICS: Mutex<ChainedPics> = {
    // # Safety
    // [pic8259_simple] didn't specify why this function is unsafe. One possible reason is the two
    // set of slots inferred from the offsets must not overlap.
    let chain_pics = unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) };
    Mutex::new(chain_pics)
};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // all interruption handlers should use the `x86-interrupt` FFI, the normal calling
        // convention assuming caller / callee-saved registers and CPU flag consistency is not
        // suitable as CPU interruption may be triggered by any instruction and change the state of
        // CPU flags.

        // CPU interrupts
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
        idt.page_fault.set_handler_fn(page_fault_handler);

        // hardware interrupts
        idt[InterruptIndex::Timer.to_usize()].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard.to_usize()].set_handler_fn(keyboard_interrupt_handler);

        idt
    };

    static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = {
        let keyboard = Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore);
        Mutex::new(keyboard)
    };
}

/// Initialize the Interrupt Description Table. Currently the following handlers are defined:
/// - breakpoint
/// - double fault
/// - timer
/// - keyboard
///
/// # Safety
/// This function is unsafe because the IDT refers to an entry in the Interrupt Stack Table which
/// must be initialized by the GDT. Calling this function before [gdt::init] refers to uninitialized
/// memory.
pub unsafe fn init_idt() {
    IDT.load();
}

/// Initialize and enable hardware interrupts in the CPU.
pub fn init_pics() {
    // # Safety
    // Again [pic8259_simple] didn't specify why this function is unsafe. It may be its unsafe usage
    // of CPU ports, which should be safe to read and write in our environment, a virtual x86_64 CPU
    // in QEMU.
    unsafe {
        PICS.lock().initialize();
    }

    // enable hardware interrupts in the CPU by `sti` instruction
    x86_64::instructions::interrupts::enable();
}

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
/// Indices into the Interrupt Descriptor Table of the interrupts originated from outside of the
/// CPU. Should match the spec of 8259 PIC and the offset used on its initialization.
#[allow(missing_docs)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard = PIC_1_OFFSET + 1,
}

impl InterruptIndex {
    /// Convert the interrupt index to u8.
    ///
    /// According to [Rust naming convention
    /// guidelines](https://rust-lang.github.io/api-guidelines/naming.html), convertion between
    /// owned Copy types should be to_*.
    pub fn to_u8(self) -> u8 {
        self as u8
    }

    /// Convert the interrupt index to usize.
    pub fn to_usize(self) -> usize {
        usize::from(self.to_u8())
    }
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) -> ! {
    panic!(
        "EXCEPTION: DOUBLE FAULT\nerror code: {}\n{:#?}",
        error_code, stack_frame
    );
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // print!(".");

    // # Safety
    // Timer is exactly the interrupt handled by this handler.
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.to_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    const PS2_KEYBOARD_PORT: u16 = 0x60;

    // let mut keyboard = KEYBOARD.lock();
    let mut port = Port::<u8>::new(PS2_KEYBOARD_PORT);

    // # Safety
    // 0x60 is the PS/2 controller data port, the port has data size of 1.
    let scancode = unsafe { port.read() };
    crate::task::keyboard::add_scancode(scancode);

    // // Processing a byte read from the PS/2 data port may not always be successful: the scancode may
    // // be invalid, the scancode may lead to an impossible state assuming the keyboard layout, the
    // // scancode may be corrupted by transmission, etc. Processing a byte may also not return a key
    // // event, e.g. the escape byte before extended keycode.
    // if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
    //     // Press and release are two separate events in IBM XT. Here only key presses are mapped to
    //     // characters.
    //     if let Some(key) = keyboard.process_keyevent(key_event) {
    //         match key {
    //             DecodedKey::RawKey(key) => print!("{:?}", key),
    //             DecodedKey::Unicode(code) => print!("{}", code),
    //         }
    //     }
    // }

    // # Safety
    // Keyboard is exactly the interrupt handled by this handler.
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.to_u8());
    }
}

#[cfg(test)]
mod tests {
    #[test_case]
    fn breakpoint_exception_handled() {
        x86_64::instructions::interrupts::int3();
        // kernel should not be terminated
    }
}
