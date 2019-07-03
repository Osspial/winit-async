#![feature(async_await)]
pub mod future;
mod async_driver;

use std::{
    cell::RefCell,
    future::Future,
    ptr,
    rc::Rc,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    time::Instant,
};
use winit::{
    event::{Event, DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowId,
};

pub struct EventLoopRunnerAsync<E: 'static> {
    shared_state: Rc<RefCell<SharedState<E>>>,
}

struct SharedState<E: 'static> {
    next_event: Option<AsyncDriverEvent<E>>,
    control_flow: Option<ptr::NonNull<ControlFlow>>,
}

enum AsyncDriverEvent<E> {
    Event(EventAsync<E>),
    Redraw(WindowId),
    TryWait,
}

struct AsyncEventSink<E: 'static, F: Future<Output=()>> {
    shared_state: Rc<RefCell<SharedState<E>>>,
    winit_event: Rc<RefCell<Option<Event<E>>>>,
    event_dest: F,
}

impl<'a, E> async_driver::EventSink<'a> for AsyncEventSink<E> {
    type Event = E;
    type DispatchEvent = dyn Future<Output=()>;
    type DispatchRedraw = dyn Future<Output=()>;
    type TryWait = dyn Future<Output=()>;

    fn dispatch_event(&'a mut self, event: EventAsync<Self::Event>) -> Self::DispatchEvent {
        unimplemented!()
    }
    fn dispatch_redraw(&'a mut self, window: WindowId) -> Self::DispatchRedraw
        unimplemented!()
    }
    fn try_wait(&'a mut self) -> Self::TryWait {
        unimplemented!()
    }
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
        event: WindowEvent,
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
        Fu: Future<Output=()>;
}

impl<E: 'static + std::fmt::Debug> EventLoopAsync for EventLoop<E> {
    type Event = E;

    fn run_async<Fn, Fu>(self, event_handler: Fn) -> !
    where
        Fn: 'static + FnOnce(EventLoopRunnerAsync<E>) -> Fu,
        Fu: Future<Output=()>
    {
        let shared_state = Rc::new(RefCell::new(SharedState {
            next_event: None,
            control_flow: None,
        }));
        let shared_state_clone = shared_state.clone();

        let winit_event: Rc<RefCell<Option<Event<E>>>> = Rc::new(RefCell::new(None));
        let winit_event_cloned = winit_event.clone();

        let mut future = Box::pin(async move {
            let runner = EventLoopRunnerAsync {
                shared_state: shared_state_clone,
            };
            event_handler(runner).await
        });
        let waker = unsafe{ Waker::from_raw(null_waker()) };

        self.run(move |event, _, control_flow| {
            let control_flow_ptr = control_flow as *mut ControlFlow;
            drop(control_flow);
            println!("\t{:?}", event);
            {
                let mut shared_state = shared_state.borrow_mut();
                shared_state.control_flow = ptr::NonNull::new(control_flow_ptr);
            }

            if unsafe{ *control_flow_ptr } != ControlFlow::Exit {
                let mut context = Context::from_waker(&waker);
                match future.as_mut().poll(&mut context) {
                    Poll::Ready(()) => unsafe{ *control_flow_ptr = ControlFlow::Exit },
                    Poll::Pending => (),
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

    pub fn recv_events(&mut self) -> impl '_ + Future<Output=future::EventReceiver<'_, E>> {
        future::EventReceiverBuilder {
            shared_state: &self.shared_state,
        }
    }
}

fn null_waker() -> RawWaker {
    RawWaker::new(
        ptr::null(),
        VTABLE
    )
}

const VTABLE: &RawWakerVTable = &RawWakerVTable::new(
    null_waker_clone,
    null_fn,
    null_fn,
    null_fn,
);

unsafe fn null_waker_clone(_: *const ()) -> RawWaker {
    null_waker()
}

unsafe fn null_fn(_: *const ()) {}
