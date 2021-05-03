use lazy_static::lazy_static;
use x86_64::{
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    VirtAddr,
};

/// The index of the double fault stack space in the Intrrupt Stack Table.
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            // size of the double fault, heavy use of the stack on double fault handling results in
            // triple fault, or worse, slient corruption of whatever memory below the stack space.
            const STACK_SIZE: usize = 4096 * 5;
            // the stack space is placed in DATA section, if not declared as mut the stack space may
            // be placed in RODATA hence modification would cause segmentation fault
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];

            // # Safety
            // reference to STACK is unique and valid as ensured by lazy_static and the fact that
            // STACK is not visible from outside of this scope
            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            // stack grows negatively, the virtual address that should be placed in TSS is one byte
            // beyond the end of the stack space
            stack_start + STACK_SIZE
        };
        tss
    };

    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.add_entry(Descriptor::kernel_code_segment());
        let tss_selector = gdt.add_entry(Descriptor::tss_segment(&TSS));
        (gdt, Selectors { code_selector, tss_selector })
    };
}

struct Selectors {
    code_selector: SegmentSelector,
    tss_selector: SegmentSelector,
}

/// Initialize the GDT (Global Descriptor Table). Use a custom GDT as mitigation of:
/// - kernel stack overflow, by switching to a separate, sufficiently large stack on double fault
///   interrupts, kernel stack overflow no longer causes bookkeeping on an already overflowed stack
///   and a fatal triple fault
pub fn init() {
    use x86_64::instructions::segmentation::set_cs;
    use x86_64::instructions::tables::load_tss;

    let (gdt, selectors) = &*GDT;
    gdt.load();

    // [GlobalDescriptorTable::load](86_64::structures::GlobalDescriptorTable::load) does not alter
    // any of the segment registers, both the kernel code segment and the TSS must be reloaded
    // manually.
    //
    // # Safety
    // `selectors.code_selector` and `selectors::tss_selector` are valid segment descriptor returned
    // by
    // [GlobalDescriptorTable::add_entry](x86_64::structures::gdt::GlobalDescriptorTable::add_entry),
    // `selectors::tss_selector` points to a valid TSS entry in the GDT defined in lazy_static.
    unsafe {
        set_cs(selectors.code_selector);
        load_tss(selectors.tss_selector);
    }
}
