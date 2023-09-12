use core::{future::Future, pin::Pin};
use core::task::{Context, Poll};
use alloc::boxed::Box;

pub mod simple_executor;

// Newtype wrapper around a pinned, heap-allocated, and dynamically dispatched future.

pub struct Task {
    /* We require that the task returns nothing. So the only effect of the future is its
    side effects, like printing. The dyn keyword allows us to store different types of
    futures. Pin<Box> type ensures that a value cannot be moved in memory by placing it 
    on the heap and preventing the creation of &mut references to it. */
    future: Pin<Box<dyn Future<Output = ()>>>,
}

// Add a constructor so we can create basic tasks.
impl Task {
    // Add a static bound to enforce that the future outlives the task wrapping it.
    pub fn new(future: impl Future<Output = ()> + 'static) -> Task {
        Task {
            future: Box::pin(future),
        }
    }
}

impl Task {
    /* Use the Pin::as_mut method to convert the self.future field of type Pin<Box<T>> first. 
    Then we call poll on the converted self.future field and return the result. */
    fn poll(&mut self, context: &mut Context) -> Poll<()> {
        self.future.as_mut().poll(context)
    }
}