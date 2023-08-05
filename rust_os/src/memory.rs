/* Each program instance has its own page table. A pointer to the currently active table is stored in a special CPU register. 
On x86, this register is called CR3. This table is the single level-4 page table on x86-64. To prevent accessing memory 4 times
to translate each virtual address to a physical frame, the TLB exists. It is a cache populated with direct address translations
and has an eviction scheme.

The bootloader sets up a hierarchal page table for us already that allows us to work with physical addresses. Accessing physical 
memory directly is not possible when paging is active, since programs could easily circumvent memory protection and access the 
memory of other programs otherwise.

However, the kernel needs access to physical memory to create mappings for page table frames. These are needed for various tasks 
like creating a new thread.

To make page table frames accessible to our kernel, there are a number of approaches:

1. Identity mapping: Each physical frame is mapped to the virtual address of the same type.
    - Disadvantage: Difficult to find and allocate large chunks of continuous memory for our user programs.

2. Map at a fixed offset: Map the page table frames at a fixed (large) offset. Something like 10TiB. 
    - Disadvantage: Need to create new mapping every time we create a new page table and doesn't allow accessing page
      tables of other address spaces.

3. Map the complete physical memory: Map everything, including page tables, to virtual addresses. This allows the kernel
   to access arbitrary physical addresses.
    - Disadvantage: Need more page tables to store the mapping of all physical memory. However, we can use the x86-64 huge
      page tables (each page is 2MiB instead of 4KiB). These allow using fewer page tables and thus are more cache efficient
      for the TLB.
    
4. Temporary page table mappings: Map page table frames temporarily only when we need them. Uses only a single page (CR3) so
   only 511 temp mappings are possible.
   - Disadvantage: A new mapping requires changing multiple page table levels, meaning we would need to set multiple CR3 entries.

5. Recursive page table mapping: Map an entry in the level-4 page table to itself. This allows the CPU to treat a level-4 table
   like a level-1 table (if the recursive entry is accessed 3 times in the virtual address). This means that we can now read 
   and write the level 1 page table because the CPU thinks that it is the mapped frame.
   - Disadvantage: Relies on x86 architecture a lot and occupies a large amount of virtual memory.

We will proceed with approach 3 because it gives us a lot of flexibility (being able to access arbitrary physical memory from 
the kernel). */

use x86_64::structures::paging::OffsetPageTable;
use x86_64::{
    structures::paging::PageTable,
    VirtAddr,
};

pub unsafe fn init(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    /* Translating virtual to physical addresses is a common task in an OS kernel, therefore the x86_64 crate provides an 
    abstraction for it. OffsetPageTable implements the Mapper trait, which allows for functions to be executed on pages. 
    OffsetPageTable has functions like translate_page which allows you to convert a page to a frame of the same size and
    map_to, which creates a new entry in the page table. 
    
    Also, by using the translation function of the MappedPageTable type, we can spare ourselves the work of implementing 
    huge page support.*/
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

/// Returns a mutable reference to the active level 4 table.
///
/// This function is unsafe because the caller must guarantee that the
/// complete physical memory is mapped to virtual memory at the passed
/// `physical_memory_offset`. Also, this function must be only called once
/// to avoid aliasing `&mut` references (which is undefined behavior).
unsafe fn active_level_4_table(physical_memory_offset: VirtAddr)
    -> &'static mut PageTable
{
    use x86_64::registers::control::Cr3;

    let (level_4_table_frame, _) = Cr3::read();

    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_table_ptr // unsafe
}

use x86_64::{
    PhysAddr,
    structures::paging::{Page, PhysFrame, Mapper, Size4KiB, FrameAllocator}
};

/// Creates an example mapping for the given page to frame `0xb8000`.
pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe {
        // FIXME: this is not safe, we do it only for testing
        /* map_to may create one or more new page tables when mapping a new page (virtual addr) to a frame.
        That's why we need the BootInfoFrameAllocator below. */
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_to_result.expect("map_to failed").flush();
}

use bootloader::bootinfo::{MemoryMap, MemoryRegionType};

/// A FrameAllocator that returns usable frames from the bootloader's memory map.
pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map, which is passed from the bootloader.
    ///
    /// This function is unsafe because the caller must guarantee that the passed
    /// memory map is valid. The main requirement is that all frames that are marked
    /// as `USABLE` in it are really unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator {
            memory_map,
            next: 0,
        }
    }
}

impl BootInfoFrameAllocator {
    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        // get usable regions from memory map
        let regions = self.memory_map.iter();
        let usable_regions = regions
            .filter(|r| r.region_type == MemoryRegionType::Usable);
        // map each region to its address range
        let addr_ranges = usable_regions
            .map(|r| r.range.start_addr()..r.range.end_addr());
        // transform to an iterator of frame start addresses
        let frame_addresses = addr_ranges.flat_map(|r| r.step_by(4096));
        // create `PhysFrame` types from the start addresses
        frame_addresses.map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

/* Marks the BootInfoFrameAllocator as a frame allocator, allowing it to be used in the map_to function in create_example_mapping.
Implementing the FrameAllocator is unsafe because the implementer must guarantee that the allocator yields only unused frames. */
unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}