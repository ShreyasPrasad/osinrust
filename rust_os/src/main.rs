/* Remove dependence on standard lib so we can build a freestanding Rust binary that 
runs on bare metal. */
#![no_std]
/* Override default entry point for program since we don't have access to the Rust runtime. */
#![no_main]

use core::panic::PanicInfo;

#[no_mangle] // don't mangle the name of this function
pub extern "C" fn _start() -> ! {
    // this function is the entry point, since the linker looks for a function
    // named `_start` by default
    loop {}
}

// This function is called on panic - we don't want to use the standard lib one..
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}