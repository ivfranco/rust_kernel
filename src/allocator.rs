use alloc::alloc::GlobalAlloc;
use core::{alloc::Layout, ptr::null_mut};
use linked_list_allocator::LockedHeap;
use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

/// Start of the kernel heap region in the virtual address space.
pub const HEAP_START: usize = 0x4444_4444_0000;

/// Size of the kernel heap region in the virtual address space.
pub const HEAP_SIZE: usize = 100 * 1024;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/// A dummy allocator, returns error to all allocation request.
pub struct Dummy;

unsafe impl GlobalAlloc for Dummy {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        null_mut()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unreachable!("dealloc should never be called")
    }
}

/// Initialize the heap region in the virtual address space, map them to physical frames.
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        // the same as array indices, the last virtual address in the heap is off by one
        let heap_end = heap_start + HEAP_SIZE - 1u64;

        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;

        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;

        // # Safety
        // The physical frames is unique per frame allocator, the arbitrarily chosen heap region may
        // conflict with virtual memory regions defined by the bootloader, in which case
        // [Mapper::map_to] would return [MapToError::PageAlreadyMapped]. This function is only
        // called once during the initialization of the kernel, no currently in-use page could be
        // mapped to another frame this way.
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush();
        }
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}
