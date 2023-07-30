use uart_16550::SerialPort;
use spin::Mutex;
use lazy_static::lazy_static;

/* Now we wish to print test result back to the host system's console. An easy way to do this is to use a serial port,
which is an old inteface standard. QEMU can redirect the bytes to the host system's standard output. */

/* Use a lazy_static like we did for the vga buffer. 
By using lazy_static we can ensure that the init method is called exactly once on its first use. */
lazy_static! {
    pub static ref SERIAL1: Mutex<SerialPort> = {
        /* Pass the address of the first IO port of the Uart. */
        let mut serial_port = unsafe { SerialPort::new(0x3F8) };
        serial_port.init();
        Mutex::new(serial_port)
    };
}

#[doc(hidden)]
pub fn _print(args: ::core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL1.lock().write_fmt(args).expect("Printing to serial failed");
    });
}

/// Prints to the host through the serial interface.
#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*));
    };
}

/// Prints to the host through the serial interface, appending a newline.
#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*));
}

/* To see the serial output from QEMU, we need to use the -serial argument to redirect the output to stdout.
See Cargo.toml. */