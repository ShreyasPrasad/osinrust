/* Remove dependence on standard lib so we can build a freestanding Rust binary that 
runs on bare metal. */
#![no_std]
/* Override default entry point for program since we don't have access to the Rust runtime. */
#![no_main]

use core::panic::PanicInfo;

static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    /* We use the vga text buffer to print to screen. */
    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            // Write the string byte and then the color byte (cyan=0xb).
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}

// This function is called on panic - we don't want to use the standard lib one..
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

/*
    We need to recompile the Rust core library for the custom target_triple_config.json we specified because it
    normally comes precompiled with the rust installation for our default architecture. Core lib contains Result,
    Option, iterators, etc. It is implicitly linked to all no_std crates, like ours, so we need it. 
*/

/*
    To turn our compiled kernel into a bootable disk image, we need to link it with a bootloader - let's use
    the bootloader dependency which is a minimal BIOS bootloader built from Rust and inline assembly. However, we need
    to link the bootloader with our compiled kernel - but Rust doesn't support post-build scripts.

    To solve this problem, we created a tool named bootimage that first compiles the kernel and bootloader, and then 
    links them together to create a bootable disk image. 

    More specifically, bootimage does the following:

        1. It compiles our kernel to an ELF file.
        2. It compiles the bootloader dependency as a standalone executable.
        3. It links the bytes of the kernel ELF file to the bootloader.
*/