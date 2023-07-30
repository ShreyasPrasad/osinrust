use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::{println, gdt};
use lazy_static::lazy_static;

/* There's a lot of different types of CPU exceptions, such as those caused by accessing a write-only
page, or dividing by 0, or accessing a privileged instruction in user mode. 

When an exception occurs, the CPU invokes the corresponding handler function. If an error invokes there
too, a double fault exception is raised and the double fault handler is invoked. If that also errors,
the operating system reboots. 

To handle exceptions, we setup the interrupt descriptor table (IDT). The hardware uses this table directly.
Each row has the same 16-byte format, consisting of the pointer to the handler function and some extra options. 

Each exception has a predefined IDT index. Thus the hardware can automatically load the the IDT entry for each
exception. When an exception occurs*/

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // Set the handler for the breakpoint function.
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            // tell the IDT that the double fault handler should use the double fault stack when a double fault occurs
            // this allows us to catch all double faults, even kernel stack overflows
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            // set an interrupt handler for the timer interrupt
            idt[InterruptIndex::Timer.as_usize()]
                .set_handler_fn(timer_interrupt_handler); // new
            // set an interrupt handler for the keyboard interrupt
            idt[InterruptIndex::Keyboard.as_usize()]
                .set_handler_fn(keyboard_interrupt_handler);
        }
        idt
    };
}

pub fn init_idt() {
    /* The load method expects a &'static self, that is, a reference valid for the complete runtime of the program. 
    This is because the CPU will access this table and it must outlive this init function. So we make the IDT static. 
    Using static mut directly is unsafe. Instead we use lazy_static to abstract that away. */
    IDT.load();
}

/* Use the x86-interrupt calling convention to invoke the breakpoint handler. */
extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

/* The test invokes the int3 function to trigger a breakpoint exception. By checking that the execution continues afterward, 
we verify that our breakpoint handler is working correctly. */
#[test_case]
fn test_breakpoint_exception() {
    // invoke a breakpoint exception
    x86_64::instructions::interrupts::int3();
}

/* Add a handler function for double faults. Doing so prevents a loop of system reboots when the system encounters
a CPU fault that doesn't have an explicit handler function yet (a triple fault causes a reboot).

Observe that this handler is diverging unlike the breakpoint_handler; the architecture does not permit returning
from a double fault. */
extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

/* Note that a specific combination of exceptions can lead to a double fault. For example, a divide by 0 exception followed
by a general protection fault causes a double fault, but other combinations may not.  */

/* A guard page is a special memory page at the bottom of a stack that makes it possible to detect stack overflows. 
The page is not mapped to any physical frame, so accessing it causes a page fault instead of silently corrupting other memory. 
The bootloader sets up a guard page for our kernel stack, so a stack overflow causes a page fault. This eventually causes
a double fault since the page fault exception handler is called with an interrupt stack frame that still points to the guard
page. This causes a triple fault and a system reboot.*/

use pic8259::ChainedPics;
use spin::{self, Mutex};

/* 
A programmable interrupt controller (PIC) aggregates hardware interrupts and notifies the CPU. The "programmable" part refers to
the flexibility to give different hardware different levels of priority in raising an interrupt. Each controller has a command
and data port.

For the Intel 8259 PIC, the secondary PIC is routed through one of the interrupt lines of the primary PIC.
The default configuration of the PICs is not usable because it sends interrupt vector numbers in the range of 0–15 to the CPU. 
These numbers are already occupied by CPU exceptions. For example, number 8 corresponds to a double fault. So we make the range for
the PICs to be 32-47 (which fall after the 0-32 for exceptions). */
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

/* Use an InterruptIndex struct to represent each interrupt code. The Timer is the first interrupt line to the PIC, so it has 
the starting index of PIC_1_OFFSET. */
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    // Use offset 33 for keyboard interrupts
    Keyboard
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

use crate::print;

/* Define an interrupt handler for the timer interrupt so we can run our kernel without crashes. The CPU treats internal
and external interrupts the same way (i.e with the same InterruptStackFrame arg). 

When we run the code with this handler, we see that the code only prints a single dot. The reason is that the PIC expects an 
explicit End Of Interrupt (EOI) signal from the handler. This tells the controller that the interrupt was processed and we
can accept another of the same type. */
extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    /* Notify the PIC that the interrupt was handled. The notify_end_of_interrupt method determines if the primary of secondary
    PIC sent the interrupt. It then sends the EOI using the CMD and DATA ports of the respective controller. The operation is
    unsafe because we can notify with the wrong interrupt index and cause the kernel to hang as a result. */
    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

/* We can cause a deadlock by adding a print statement to an interrupt, since the underlying writer may already be locked by 
the kernel before the interrupt is raised (so the interrupt can never acquire the writer lock). To fix this, we can disable
interrupts as long as the writer is locked (see vga_buffer.rs). Interrupts should only ever be disabled for a short time to
reduce the worse case interrupt latency. */

/* Let's add an interrupt handler function for keyboard interrupts so can we can catch the keystroke events that are already
sent to the CPU. */
extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    /* To find out which key was pressed, we need to read the query the keyboard controller. We do this by reading the data port
    of the PS/2 controller which is the I/O port with number 0x60. */
    use x86_64::instructions::port::Port;
    // Use the scancode converter of an external crate rather than writing our own
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(layouts::Us104Key, ScancodeSet1,
                HandleControl::Ignore)
            );
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);

    let scancode: u8 = unsafe { port.read() };
    // Convert the scancode to a keyevent, which contains the type of key event (press or release) as well as the key itself.
    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        // Tell the keyboard to process the keyevent and produce a decoded key that we output.
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => print!("{}", character),
                DecodedKey::RawKey(key) => print!("{:?}", key),
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}