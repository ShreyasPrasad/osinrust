use alloc::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

pub struct Dummy;

/* The GlobalAlloc trait must be implemented to support dynamic memory allocation and deallocation
for heap memory. The standard lib has an implementation, but in our no_std envirionment, we provide
a custom implementation that the alloc crate can use. 

This implementation is a simple, dummy one. */
unsafe impl GlobalAlloc for Dummy {
    
    /* The alloc method takes a Layout instance as an argument, which describes the desired size and 
    alignment that the allocated memory should have. It returns a raw pointer to the first byte of the 
    allocated memory block. */
    unsafe fn alloc(&self, _layout: Layout) -> *mut u8 {
        null_mut()
    }

    /* The dealloc method is the counterpart and is responsible for freeing a memory block again. 
    It receives two arguments: the pointer returned by alloc and the Layout that was used for the allocation. */
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        panic!("dealloc should be never called")
    }
}

/* The #[global_allocator] attribute tells the Rust compiler which allocator instance it should use as the 
global heap allocator. The attribute is only applicable to a static that implements the GlobalAlloc trait.  */
use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOCATOR: LockedHeap = LockedHeap::empty();

/* To create a kernel heap, we need to define a heap memory region from which the allocator can allocate memory.
To do this, we need to define a virtual memory range for the heap region and then map this region to physical frames. */

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

use x86_64::{
    structures::paging::{
        mapper::MapToError, FrameAllocator, Mapper, Page, PageTableFlags, Size4KiB,
    },
    VirtAddr,
};

/* Create the kernel heap. The function takes mutable references to a Mapper and a FrameAllocator instance, 
both limited to 4 KiB pages by using Size4KiB as the generic parameter. */
pub fn init_heap(
    mapper: &mut impl Mapper<Size4KiB>,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) -> Result<(), MapToError<Size4KiB>> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(MapToError::FrameAllocationFailed)?;
        /* With these flags, both read and write accesses are allowed, which makes sense for heap memory. */
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper.map_to(page, frame, flags, frame_allocator)?.flush()
        };
    }

    /* Initialize the allocator after allocating the heap frames because the init() method writes to the heap. */
    unsafe {
        ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE);
    }

    Ok(())
}