use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};
use crate::println;
use lazy_static::lazy_static;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        // Set the handler for the breakpoint function.
        idt.breakpoint.set_handler_fn(breakpoint_handler);
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