#![no_std]
#![no_main]

use core::panic::PanicInfo;
use rust_os::{QemuExitCode, exit_qemu, serial_println, serial_print};

/* This test uses the harness=false flag in Cargo.toml to disable the default and custom test runner.
We run the test directly from the _start entry point. */
#[no_mangle]
pub extern "C" fn _start() -> ! {
    should_fail();
    serial_println!("[test did not panic]");
    exit_qemu(QemuExitCode::Failed);
    loop{}
}

fn should_fail() {
    serial_print!("should_panic::should_fail...\t");
    assert_eq!(0, 1);
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    serial_println!("[ok]");
    exit_qemu(QemuExitCode::Success);
    loop {}
}