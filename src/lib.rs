#![feature(async_await)]
pub mod future;
mod async_driver;

use std::{
    cell::RefCell,
    future::Future,
    ptr,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    time::Instant,
};
use winit::{
    event::{Event, DeviceEvent, DeviceId, StartCause, WindowEvent},
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

#[derive(Debug)]
enum AsyncDriverEvent<E: 'static> {
    Event(EventAsync<E>),
    EventsCleared,
    Redraw(WindowId),
    RedrawsCleared,
    SetWait,
    WakeUp(StartCause),
}

impl<E> AsyncDriverEvent<E> {
    fn map_nonuser_event<T>(self) -> Result<AsyncDriverEvent<T>, AsyncDriverEvent<E>> {
        match self {
            AsyncDriverEvent::Event(event) => event.map_nonuser_event().map(AsyncDriverEvent::Event).map_err(AsyncDriverEvent::Event),
            AsyncDriverEvent::EventsCleared => Ok(AsyncDriverEvent::EventsCleared),
            AsyncDriverEvent::Redraw(window_id) => Ok(AsyncDriverEvent::Redraw(window_id)),
            AsyncDriverEvent::RedrawsCleared => Ok(AsyncDriverEvent::RedrawsCleared),
            AsyncDriverEvent::SetWait => Ok(AsyncDriverEvent::SetWait),
            AsyncDriverEvent::WakeUp(cause) => Ok(AsyncDriverEvent::WakeUp(cause)),
        }
    }
}

struct AsyncEventSink<E: 'static, F: Unpin + Future<Output=()>> {
    shared_state: Rc<RefCell<SharedState<E>>>,
    event_dest: F,
}

impl<'a, E, F> async_driver::EventSink<'a> for AsyncEventSink<E, F>
    where F: Unpin + Future<Output=()>,
{
    type Event = E;

    fn dispatch_event(&'a mut self, event: EventAsync<Self::Event>) -> bool {
        let waker = unsafe{ Waker::from_raw(null_waker()) };
        let mut context = Context::from_waker(&waker);
        self.shared_state.borrow_mut().next_event = Some(AsyncDriverEvent::Event(event));
        // println!("dispatch_event");

        match Pin::new(&mut self.event_dest).poll(&mut context) {
            Poll::Ready(()) => true,
            Poll::Pending   => false,
        }
    }
    fn events_cleared(&'a mut self) -> bool {
        let waker = unsafe{ Waker::from_raw(null_waker()) };
        let mut context = Context::from_waker(&waker);
        self.shared_state.borrow_mut().next_event = Some(AsyncDriverEvent::EventsCleared);
        // println!("events_cleared");

        match Pin::new(&mut self.event_dest).poll(&mut context) {
            Poll::Ready(()) => true,
            Poll::Pending   => false,
        }
    }
    fn dispatch_redraw(&'a mut self, window: WindowId) -> bool {
        let waker = unsafe{ Waker::from_raw(null_waker()) };
        let mut context = Context::from_waker(&waker);
        self.shared_state.borrow_mut().next_event = Some(AsyncDriverEvent::Redraw(window));
        // println!("dispatch_redraw");

        match Pin::new(&mut self.event_dest).poll(&mut context) {
            Poll::Ready(()) => true,
            Poll::Pending   => false,
        }
    }
    fn redraws_cleared(&'a mut self) -> bool {
        let waker = unsafe{ Waker::from_raw(null_waker()) };
        let mut context = Context::from_waker(&waker);
        self.shared_state.borrow_mut().next_event = Some(AsyncDriverEvent::RedrawsCleared);
        println!("redraws_cleared");

        match Pin::new(&mut self.event_dest).poll(&mut context) {
            Poll::Ready(()) => true,
            Poll::Pending   => false,
        }
    }
    fn set_wait(&'a mut self) -> bool {
        let waker = unsafe{ Waker::from_raw(null_waker()) };
        let mut context = Context::from_waker(&waker);
        self.shared_state.borrow_mut().next_event = Some(AsyncDriverEvent::SetWait);
        // println!("set_wait");

        match Pin::new(&mut self.event_dest).poll(&mut context) {
            Poll::Ready(()) => true,
            Poll::Pending   => false,
        }
    }
    fn wake_up(&'a mut self, start_cause: StartCause) -> bool {
        let waker = unsafe{ Waker::from_raw(null_waker()) };
        let mut context = Context::from_waker(&waker);
        self.shared_state.borrow_mut().next_event = Some(AsyncDriverEvent::WakeUp(start_cause));
        // println!("wake_up");

        match Pin::new(&mut self.event_dest).poll(&mut context) {
            Poll::Ready(()) => true,
            Poll::Pending   => false,
        }
    }
}

#[must_use]
pub enum WaitCancelledCause {
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

impl<E> EventAsync<E> {
    pub fn map_nonuser_event<U>(self) -> Result<EventAsync<U>, EventAsync<E>> {
        use self::EventAsync::*;
        match self {
            UserEvent(_) => Err(self),
            WindowEvent { window_id, event } => Ok(WindowEvent { window_id, event }),
            DeviceEvent { device_id, event } => Ok(DeviceEvent { device_id, event }),
            Suspended => Ok(Suspended),
            Resumed => Ok(Resumed),
        }
    }
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
                shared_state: shared_state_clone.clone(),
            };
            let event_source = || future::PendingWhileNone::new(winit_event_cloned.clone());
            let event_dest = Box::pin(event_handler(runner));

            let event_sink = AsyncEventSink {
                shared_state: shared_state_clone.clone(),
                event_dest: event_dest,
            };
            async_driver::async_driver(event_source, event_sink).await
        });
        let waker = unsafe{ Waker::from_raw(null_waker()) };

        self.run(move |event, _, control_flow| {
            let control_flow_ptr = control_flow as *mut ControlFlow;
            drop(control_flow);
            {
                let mut shared_state = shared_state.borrow_mut();
                shared_state.control_flow = ptr::NonNull::new(control_flow_ptr);
            }

            println!("{:?}", event);
            *winit_event.borrow_mut() = Some(event);

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
            dispatch_redraws: false,
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
