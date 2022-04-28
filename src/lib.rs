//! This crate implements an asynchronous version of the winit `EventLoop`, thus allowing proper usage of [`tokio`] on the client side.
//!
//! ```rust
//! pub fn main() {
//!     // ...
//! }
//! ```

pub mod future;

use std::{
    cell::RefCell,
    future::Future,
    ptr,
    rc::Rc,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    time::Instant,
};

// We need this stuff locally, so we write it out.
use winit::{
    event::{DeviceEvent, DeviceId, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowId,
};

// Re-export all of winit.
// While this does make the above imports pointless, we keep them for clarity.
pub use winit;
pub use raw_window_handle::*;

pub struct EventLoopRunnerAsync<E: 'static> {
    shared_state: Rc<RefCell<SharedState<E>>>,
}

struct SharedState<E: 'static> {
    next_event: Option<Event<'static, E>>,
    control_flow: Option<ptr::NonNull<ControlFlow>>,
}

#[must_use]
pub enum WaitCanceledCause {
    ResumeTimeReached,
    EventsReceived,
}

#[derive(Clone, Debug, PartialEq)]
pub enum EventAsync<E: 'static> {
    WindowEvent {
        window_id: WindowId,
        event: WindowEvent<'static>,
    },
    DeviceEvent {
        device_id: DeviceId,
        event: DeviceEvent,
    },
    UserEvent(E),
    Suspended,
    Resumed,
}

pub trait EventLoopAsync {
    type Event: 'static;
    fn run_async<Fn, Fu>(self, event_handler: Fn) -> !
    where
        Fn: 'static + FnOnce(EventLoopRunnerAsync<Self::Event>) -> Fu,
        Fu: Future<Output = ()>;
}

impl<E: 'static + std::fmt::Debug> EventLoopAsync for EventLoop<E> {
    type Event = E;

    /// The actual winit mainloop, made asynchronous.
    fn run_async<Fn, Fu>(self, event_handler: Fn) -> !
    where
        Fn: 'static + FnOnce(EventLoopRunnerAsync<E>) -> Fu,
        Fu: Future<Output = ()>,
    {
        let shared_state = Rc::new(RefCell::new(SharedState {
            next_event: None,
            control_flow: None,
        }));
        let shared_state_clone = shared_state.clone();
        
        use std::sync::{atomic::AtomicBool, atomic::Ordering, Arc};
        let completion = Arc::from(AtomicBool::new(false));
        let completion_ref = completion.clone();
        
        // This is what wraps the loop.
        let mut future = Box::pin(async move {
            let runner = EventLoopRunnerAsync {
                shared_state: shared_state_clone,
            };
            event_handler(runner).await;
            completion_ref.store(true, Ordering::Relaxed);
        });
        let waker = unsafe { Waker::from_raw(null_waker()) };

        self.run(move |event, _, control_flow| {
            // Change to polling mode.
            *control_flow = ControlFlow::Poll;

            let control_flow_ptr = control_flow as *mut ControlFlow;
            
            // This *must* happen. Please don't ask why.
            #[allow(clippy::drop_ref)]
            drop(control_flow);
            
            {
                let mut shared_state = shared_state.borrow_mut();
                shared_state.control_flow = ptr::NonNull::new(control_flow_ptr);

                if let Some(event) = event.to_static() {
                    shared_state.next_event = Some(event);
                }
            }

            if unsafe { *control_flow_ptr } != ControlFlow::Exit {
                let mut context = Context::from_waker(&waker);
                
                // Check if the Future has completed...
                if completion.load(Ordering::Relaxed) {
                    unsafe {
                        *control_flow_ptr = ControlFlow::Exit;
                    }
                } else {
                    // ...now we can poll the Future without causing a panic.
                    match future.as_mut().poll(&mut context) {
                        Poll::Ready(()) => unsafe { *control_flow_ptr = ControlFlow::Exit },
                        Poll::Pending => (),
                    }
                }
            }
            
            shared_state.borrow_mut().control_flow = None;
        });
    }
}

impl<E> EventLoopRunnerAsync<E> {
    pub fn wait(&mut self) -> future::WaitFuture<'_, E> {
        future::WaitFuture {
            shared_state: &self.shared_state,
        }
    }

    pub fn wait_until(&mut self, timeout: Instant) -> future::WaitUntilFuture<'_, E> {
        future::WaitUntilFuture {
            timeout,
            shared_state: &self.shared_state,
        }
    }

    pub fn recv_events(&mut self) -> impl '_ + Future<Output = future::EventReceiver<'_, E>> {
        future::EventReceiverBuilder {
            shared_state: &self.shared_state,
        }
    }
}

fn null_waker() -> RawWaker {
    RawWaker::new(ptr::null(), VTABLE)
}

const VTABLE: &RawWakerVTable = &RawWakerVTable::new(null_waker_clone, null_fn, null_fn, null_fn);

unsafe fn null_waker_clone(_: *const ()) -> RawWaker {
    null_waker()
}

unsafe fn null_fn(_: *const ()) {}
