/*  A fixed-size block allocator consists of power-of-2 block-size allocations. For example, if we wish to
allocate 6 bytes, an 8 byte block would be allocated. Adding more possible block sizes reduces the extent
of memory wastage. We would have a head pointer for the block of each size. 
This makes allocations and deallocations very efficient.

Given that large allocations (>2 KB) are often rare, especially in operating system kernels, it might make 
sense to fall back to a different allocator for these allocations. For example, we could fall back to a 
linked list allocator for allocations greater than 2048 bytes in order to reduce memory waste. 

When we run out of blocks of a particular size, we can either revert to our fallback allocator or we can
split a larger block into smaller blocks (easy for powers of 2). In our implementation,  we will just revert
to the fallback - it's easier.

This approach performs better then the linked list implementation but wastes up to half the memory when using
powers of 2 as the block size. Some things we can do to improve the implementation:

1. Preallocate blocks for each block size to make initial allocations more performant.
2. Support the alignment of other block sizes.
3. Enforce a maximum length list for each block size. When maximum is reached, fallback deallocations do not 
add to the block list.
4. For allocations larger than 4KiB, use a special paging allocator to map a large continuous virt addr range
to noncontinuous physical frames. This solves fragmentation of unused memory for large allocations. A page 
allocator like this makes worst case performance more predictable. */

use core::{alloc::{Layout, GlobalAlloc}, ptr::{self, NonNull}, mem};

use super::bump::Locked;

/// The block sizes to use.
///
/// The sizes must each be power of 2 because they are also used as
/// the block alignment (alignments must be always powers of 2).
/// We don’t define any block sizes smaller than 8 because each block must 
/// be capable of storing a 64-bit pointer to the next block when freed.
const BLOCK_SIZES: &[usize] = &[8, 16, 32, 64, 128, 256, 512, 1024, 2048];

// Similar to the linked_list ListNode except we don't have a size this time. The size isn't needed because
// the sizes of blocks are known beforehand.
struct ListNode {
    next: Option<&'static mut ListNode>,
}

pub struct FixedSizeBlockAllocator {
    // an array of head pointers, one for each block size
    list_heads: [Option<&'static mut ListNode>; BLOCK_SIZES.len()],
    fallback_allocator: linked_list_allocator::Heap,
}

impl FixedSizeBlockAllocator {
    /// Creates an empty FixedSizeBlockAllocator.
    pub const fn new() -> Self {
        const EMPTY: Option<&'static mut ListNode> = None;
        FixedSizeBlockAllocator {
            /* The EMPTY constant is needed to tell the Rust compiler that we want to initialize the array with a constant value. 
            Initializing the array directly as [None; BLOCK_SIZES.len()] does not work, because then the compiler requires 
            Option<&'static mut ListNode> to implement the Copy trait, which it does not. This is a current limitation of 
            the Rust compiler, which might go away in the future. */
            list_heads: [EMPTY; BLOCK_SIZES.len()],
            fallback_allocator: linked_list_allocator::Heap::empty(),
        }
    }

    /// Initialize the allocator with the given heap bounds.
    ///
    /// This function is unsafe because the caller must guarantee that the given
    /// heap bounds are valid and that the heap is unused. This method must be
    /// called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.fallback_allocator.init(heap_start, heap_size);
    }

    /// Allocates using the fallback allocator.
    fn fallback_alloc(&mut self, layout: Layout) -> *mut u8 {
        match self.fallback_allocator.allocate_first_fit(layout) {
            Ok(ptr) => ptr.as_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }
}

/// Choose an appropriate block size for the given layout.
///
/// Returns an index into the `BLOCK_SIZES` array.
fn list_index(layout: &Layout) -> Option<usize> {
    let required_block_size = layout.size().max(layout.align());
    BLOCK_SIZES.iter().position(|&s| s >= required_block_size)
}

unsafe impl GlobalAlloc for Locked<FixedSizeBlockAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                match allocator.list_heads[index].take() {
                    Some(node) => {
                        allocator.list_heads[index] = node.next.take();
                        node as *mut ListNode as *mut u8
                    }
                    None => {
                        // no block exists in list => allocate new block
                        let block_size = BLOCK_SIZES[index];
                        // only works if all block sizes are a power of 2
                        let block_align = block_size;
                        /* The reason we create a new layout is so that we can add the block to the list when it is freed. */
                        let layout = Layout::from_size_align(block_size, block_align)
                            .unwrap();
                        allocator.fallback_alloc(layout)
                    }
                }
            }
            None => allocator.fallback_alloc(layout),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut allocator = self.lock();
        match list_index(&layout) {
            Some(index) => {
                let new_node = ListNode {
                    next: allocator.list_heads[index].take(),
                };
                // verify that block has size and alignment required for storing node
                assert!(mem::size_of::<ListNode>() <= BLOCK_SIZES[index]);
                assert!(mem::align_of::<ListNode>() <= BLOCK_SIZES[index]);
                let new_node_ptr = ptr as *mut ListNode;
                new_node_ptr.write(new_node);
                /* The last step is to set the head pointer of the list, which is currently None since we called 
                take on it, to our newly written ListNode. For that, we convert the raw new_node_ptr to a mutable reference. */
                allocator.list_heads[index] = Some(&mut *new_node_ptr);
            }
            None => {
                let ptr = NonNull::new(ptr).unwrap();
                allocator.fallback_allocator.deallocate(ptr, layout);
            }
        }
    }
}