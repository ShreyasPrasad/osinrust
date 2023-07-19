#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(rust_os::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;
use rust_os::println;

/* All integration tests are their own executables and completely separate from our main.rs. 
This means that each test needs to define its own entry point function. */
#[no_mangle]
pub extern "C" fn _start() -> ! {
    test_main();

    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    rust_os::test_panic_handler(info)
}

/* Make sure printing works after a basic boot so it is something we can depend on in more complicated tests. */
#[test_case]
fn test_println() {
    println!("test_println output");
}