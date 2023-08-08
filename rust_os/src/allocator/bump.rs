/* The BumpAllocator is the simplest type of heap allocator and it allocates memory by bumping a "next" pointer which
points to the next available heap address for an allocation. On each allocation, next is increased by the allocation size. 

Advantages: Very fast as it consists of only a few assembly instructions. Simple.
Disadvantages: Memory can only be reused once all allocations have been freed.

The fundamental problem with the bump allocator is it can never keep track of an arbitrary number of unused memory regions
(allocations can be arbitrarily freed in any order). Just one next_pointer can't encapsulate all that information.
*/

use core::alloc::GlobalAlloc;
use core::ptr;
use super::align_up;

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    /* next always points to the next unused address in heap memory. */
    next: usize,
    /* On each alloc, this is incremented by 1. On each dealloc, this is decremented by 1. When it reaches 0, we set the 
    next pointer to the heap start address. */
    allocations: usize,
}

impl BumpAllocator {
    /// Creates a new empty bump allocator.
    /// Make this a const function so its evaluable at compile time for the static allocator to be declared.
    pub const fn new() -> Self {
        BumpAllocator {
            heap_start: 0,
            heap_end: 0,
            next: 0,
            allocations: 0,
        }
    }

    /// Initializes the bump allocator with the given heap bounds.
    ///
    /// This method is unsafe because the caller must ensure that the given
    /// memory range is unused. Also, this method must be called only once.
    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }
}

/* Rust does not permit implementing traits for types defined in other crates. So we 
create a locking wrapper. */
pub struct Locked<A> {
    inner: spin::Mutex<A>
}

impl<A> Locked<A> {
    // Make this a const function so its evaluable at compile time for the static allocator to be declared.
    pub const fn new(inner: A) -> Self {
        Locked {
            inner: spin::Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> spin::MutexGuard<A> {
        self.inner.lock()
    }
}

/* All the methods of GlobalAlloc only take an immutable &self reference. */
unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let mut bump = self.lock();
        let alloc_start = align_up(bump.next, layout.align());
        /* To prevent integer overflow on large allocations, we use the checked_add method. */
        let alloc_end = match alloc_start.checked_add(layout.size()) {
            Some(end) => end,
            None => return ptr::null_mut(),
        };

        if alloc_end > bump.heap_end {
            ptr::null_mut() // out of memory
        } else {
            bump.next = alloc_end;
            bump.allocations += 1;
            alloc_start as *mut u8
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let mut bump = self.lock(); // get a mutable reference

        bump.allocations -= 1;
        if bump.allocations == 0 {
            bump.next = bump.heap_start;
        }
    }
}