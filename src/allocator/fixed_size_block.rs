use core::{
    alloc::{GlobalAlloc, Layout},
    mem,
    ptr::{null_mut, NonNull},
};

use crate::locked::Locked;

/// The block sizes to use. To simplify the implementation each block has alignment equal to its
/// size, as a consequence the block sizes defined here must be a power of 2.
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

struct ListNode {
    /// A owned list node on memory not managed by Rust ownership system
    next: Option<&'static mut ListNode>,
}

/// A fixed-size block allocator, maintains multiple node lists of same sized memory chunks.
pub struct FixedSizeBlockAllocator {
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}

impl FixedSizeBlockAllocator {
    /// Construct a new empty fixed-size block allocator.
    #[allow(clippy::new_without_default)]
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;

        FixedSizeBlockAllocator {
            // how is the uniqueness of the possible mutable reference guaranteed in this case?
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// # Safety
    /// This function is unsafe because the caller must guarantee that the given heap bounds are
    /// valid and that the heap is unused. This method must be called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback_allocator.init(heap_start, heap_size);
    }

    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => null_mut(),
        }
    }
}

/// Choose a proper block size for the given layout.
///
/// Returns an index into the [BLOCK_SIZES] array, `None` if no block size in [BLOCK_SIZES] can fit
/// the required layout.
fn list_index(layout: &Layout) -> Option<usize> {
    // to make sure the alignment of the block is greater or equal to the alignment of the layout
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES
        .iter()
        .position(|&size| size >= required_block_size)
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();

        match list_index(&layout) {
            Some(index) => match allocator.list_heads[index].take() {
                Some(node) => {
                    allocator.list_heads[index] = node.next.take();
                    // should be fine, the alignment of any type is a multiple of 1
                    node as *mut ListNode as *mut u8
                }
                None => {
                    // the required node list is empty, no free block has the required size
                    let block_size = BLOCK_SIZES[index];
                    // works because how BLOCK_SIZES is defined: every entry is a power of 2
                    let layout = Layout::from_size_align(block_size, block_size).unwrap();
                    // the block is instead allocated from the fallback allocator
                    allocator.fallback_alloc(layout)
                }
            },
            None => {
                // the required layout doesn't fit in any predefined block size
                allocator.fallback_alloc(layout)
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                let new_node = ListNode {
                    next: allocator.list_heads[index].take(),
                };
                // all allocation ultimately came from the fallback allocator, both the size and
                // alignment of those allocations should be `BLOCK_SIZES[index]`
                //
                // there's enough memory to write
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                // the write is aligned
                assert!(BLOCK_SIZES[index] % mem::align_of::<ListNode>() == 0);

                let new_node_ptr = ptr as *mut ListNode;
                // # Safety
                // As asserted above,
                // - the write is correctly aligned for [ListNode]
                // - the writable region is no smaller than size of [ListNode]
                new_node_ptr.write(new_node);

                // # Safety
                // A valid instance of [ListNode] is written to the pointer right above, the
                // exclusive ownership of the reference (assume the system crates never double free)
                // is then given to the node list.
                allocator.list_heads[index] = new_node_ptr.as_mut();
            }
            None => {
                // deallocation of a massive block that doesn't belong to any node list
                let ptr = NonNull::new(ptr).expect("system crate frees null ptr");
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}
