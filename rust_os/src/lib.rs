#![no_std]

#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]
#![feature(abi_x86_interrupt)]
#![feature(const_mut_refs)]

use core::panic::PanicInfo;

use x86_64::instructions::hlt;

pub mod vga_buffer;
pub mod serial;
pub mod interrupts;
pub mod gdt;
pub mod memory;
pub mod allocator;

/* The standard library alloc crate, used for dynamic memory allocation. */
extern crate alloc;

/* Now, we implement a more robust testing framework, that inserts serial prints where necessary. */
pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        serial_print!("{}...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[failed]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[cfg(test)]
use bootloader::{entry_point, BootInfo};

#[cfg(test)]
entry_point!(test_kernel_main);

/// Entry point for `cargo test`
#[cfg(test)]
fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
    // like before
    init();
    test_main();
    hlt_loop();
}

#[cfg(test)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

/* In order to exit QEMU after tests are run automatically, we can use extra arguments (see
Cargo.toml). The bootimage runner appends the test-args to the default QEMU command for all test 
executables. For a normal cargo run, the arguments are ignored. */

/* There are 2 different approaches for communicating between CPU and peripheral hardware on x86:

    1. Memory-Mapped IO. This is what we did when we accessed the VGA buffer through a memory address explicitly.

    2. Port-Mapped IO. Uses a separate I/O bus for communication. Each connected peripheral has 1 or more port
    numbers. To communicate with such a port, there are special CPU instructions called in an out which take a 
    port number and a date byte.

The isa-debug-exit device uses port-mapped I/O. The iobase parameter specifies on which port address the device 
should live (0xf4 is a generally unused port on the x86’s IO bus) and the iosize specifies the port size (0x04 
means four bytes).

When a value is written to the port specified by iobase, it causes QEMU to exit with status equal to (value << 1) | 1.
We create the QemuExitCode u32 struct as the value to write (it is 4 bytes, just like what we specified as the iosize).
*/

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    /* We use exit codes that do not conflict with existing QEMU exit codes. */
    /* We add test-success-exit-code = 33 to Cargo.toml so that (Success << 1) | 1 = 33 is recognized as a success case. 
    It is mapped back to exit code = 0 in the context of cargo test. */
    Success = 0x10, // 16 in binary
    Failed = 0x11, // 17 in binary
}

/* The function creates a new Port at 0xf4, which is the iobase of the isa-debug-exit device. Then it writes the passed 
exit code to the port. */
pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;

    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

/* Initialize the CPU interrupt handler. */
pub fn init() {
    interrupts::init_idt();
    gdt::init();
    /* The interrupts::enable function of the x86_64 crate executes the special sti instruction to enable external hardware interrupts.  */
    unsafe { interrupts::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
}

pub fn hlt_loop() -> ! {
    // hlt: Halt the CPU until the next interrupt arrives and allow the CPu eot tner a sleep state.
    loop {
        hlt();
    }
}