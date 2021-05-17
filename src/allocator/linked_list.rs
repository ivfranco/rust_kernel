use core::{
    alloc::{GlobalAlloc, Layout, LayoutError},
    mem,
    ptr::null_mut,
};

use crate::{allocator::align_up, locked::Locked};

struct ListNode {
    size: usize,
    /// a owned ListNode, but not managed by the Rust ownership system
    next: Option<&'static mut ListNode>,
}

impl ListNode {
    const fn new(size: usize) -> Self {
        ListNode { size, next: None }
    }

    fn start_addr(&self) -> usize {
        self as *const Self as usize
    }

    fn end_addr(&self) -> usize {
        self.start_addr() + self.size
    }
}

/// A linked list allocator that embeds its data structures into free chunks. This allocator will
/// not coalesce free memory chunks.
pub struct LinkedListAllocator {
    head: ListNode,
}

impl LinkedListAllocator {
    /// Construct an empty [LinkedListAllocator]. The physical memory is not attached to this
    /// allocator at this point.
    pub const fn new() -> Self {
        Self {
            // a dummy list node with size 0, this dummy node will always be the first node in the
            // free list
            head: ListNode::new(0),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// # Safety
    /// This function is unsafe because the caller must guarantee that the given heap bounds are
    /// valid and that the heap is unused. This method must be called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.add_free_region(heap_start, heap_size)
    }

    /// Add the `size`-byte free memory region starting at `addr` to the start of the free list.
    ///
    /// # Safety
    /// This function is unsafe because the caller must guarantee that the given memory region is
    /// valid and the allocator would have exclusive access to the memory region.
    unsafe fn add_free_region(&mut self, addr: usize, size: usize) {
        // should be true for all chunks allocated and later freed by the allocator
        assert_eq!(align_up(addr, mem::align_of::<ListNode>()), Some(addr));
        assert!(size >= mem::size_of::<ListNode>());

        let mut node = ListNode::new(size);
        node.next = self.head.next.take();

        let node_ptr = addr as *mut ListNode;
        // # Safety
        // Safety requirements of this function ensured `node_ptr` is valid to write.
        //
        // The asserts at the beginning of this function ensured `node_ptr` is properly aligned.
        node_ptr.write(node);
        // # Safety
        // The only place where a [ListNode] is created in the memory is right above, the only
        // reference created that way points to a valid instance of [NodeList].
        //
        // The instance is only invalidated after its references removed from the free list, in
        // all cases references to [ListNode] existing in the free list point to valid instances.
        self.head.next = node_ptr.as_mut();
    }

    /// Looks for a free region with the given size and alignment and removes it from the list.
    ///
    /// Returns a tuple of the list node and the start address of the allocation.
    fn find_region(&mut self, layout: Layout) -> Option<(&'static mut ListNode, usize)> {
        let mut current = &mut self.head;

        while let Some(ref mut region) = current.next {
            if let Some(alloc_start) = alloc_from_region(region, layout) {
                // remove the chosen node from the free list
                let next = region.next.take();
                let ret = Some((current.next.take().unwrap(), alloc_start));
                current.next = next;
                return ret;
            } else {
                current = current.next.as_mut().unwrap();
            }
        }

        None
    }
}

fn alloc_from_region(region: &ListNode, layout: Layout) -> Option<usize> {
    let alloc_start = align_up(region.start_addr(), layout.align())?;
    let alloc_end = alloc_start.checked_add(layout.size())?;

    if alloc_end > region.end_addr() {
        // region too small for the required allocation
        return None;
    }

    let excess_size = region.end_addr() - alloc_end;
    if excess_size > 0 && excess_size < mem::size_of::<ListNode>() {
        // Rest of the region after the allocation too small for a ListNode. Currently the allocator
        // always allocates an exact size chunk on request and splits the chunk after an allocation,
        // In practice instead of being deemed invalid for the layout, this bigger than requested
        // chunk would be allocated to the caller without being split.
        None
    } else {
        Some(alloc_start)
    }
}

fn size_align(layout: Layout) -> Result<Layout, LayoutError> {
    // each allocated chunk starts with a [ListNode]
    let layout = layout.align_to(mem::align_of::<ListNode>())?.pad_to_align();
    // the allocated chunk must be big enough for the [ListNode] header
    let size = layout.size().max(mem::size_of::<ListNode>());
    Layout::from_size_align(size, layout.align())
}

unsafe impl GlobalAlloc for Locked<LinkedListAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let layout = match size_align(layout) {
            Ok(layout) => layout,
            Err(_) => return null_mut(),
        };
        let mut allocator = self.lock();

        match allocator.find_region(layout) {
            Some((region, alloc_start)) => {
                let alloc_end = alloc_start
                    .checked_add(layout.size())
                    .expect("allocation overflow");
                let excess_size = region.end_addr() - alloc_end;
                if excess_size > 0 {
                    allocator.add_free_region(alloc_end, excess_size);
                }
                alloc_start as *mut u8
            }
            None => null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let aligned = size_align(layout).expect("invalid layout returned from user");
        self.lock().add_free_region(ptr as usize, aligned.size());
    }
}
