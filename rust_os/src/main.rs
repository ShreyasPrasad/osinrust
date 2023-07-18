/* Remove dependence on standard lib so we can build a freestanding Rust binary that 
runs on bare metal. */
#![no_std]
/* Override default entry point for program since we don't have access to the Rust runtime. */
#![no_main]

use core::panic::PanicInfo;

mod vga_buffer;

/*
    To print a character to the screen in VGA text mode, one has to write it to the text buffer of the VGA hardware. 
    The VGA text buffer is a two-dimensional array with typically 25 rows and 80 columns, which is directly rendered to the screen.
*/

#[no_mangle]
pub extern "C" fn _start() -> ! {
    println!("Hello World{}", "!");
    loop {}
}

// This function is called on panic - we don't want to use the standard lib one..
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);

    loop {}
}