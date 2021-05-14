use core::{
    alloc::{GlobalAlloc, Layout},
    ptr::null_mut,
};

use crate::locked::Locked;

use super::align_up;

/// A bump allocator that never frees memory.
pub struct BumpAllocator {
    /// start of the heap region
    heap_start: usize,
    /// one byte beyond the heap region
    heap_end: usize,
    /// always points to the next unused byte in the region
    next: usize,
    /// number of fulfilled allocations
    allocations: usize,
}

impl BumpAllocator {
    /// Initializes the bump allocator with the given heap bounds.
    ///
    /// # Safety
    /// This method is unsafe because the caller must ensure that the given memory range is unused.
    /// Also, this method must not be called more than once on the same region of virtual memory.
    pub const unsafe fn new(heap_start: usize, heap_size: usize) -> Self {
        BumpAllocator {
            heap_start,
            heap_end: heap_start + heap_size,
            next: heap_start,
            allocations: 0,
        }
    }
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut bump = self.lock();

        let alloc_start = match align_up(bump.next, layout.align()) {
            Some(start) => start,
            None => return null_mut(),
        };
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return null_mut(),
        };

        if alloc_end > bump.heap_end {
            // out of memory
            null_mut()
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        let mut bump = self.lock();

        // bump allocator does not check whether the free request is valid.
        bump.allocations -= 1;
        if bump.allocations == 0 {
            // free all memory after every allocation in the heap was freed
            bump.next = bump.heap_start;
        }
    }
}
