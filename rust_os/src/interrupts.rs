use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::{println, gdt};
use lazy_static::lazy_static;

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

