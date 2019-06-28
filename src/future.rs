use crate::{
    SharedState,
    EventAsync,
    WaitCanceledCause,
};
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    time::Instant,
};
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::ControlFlow,
    window::WindowId,
};

#[must_use]
pub struct WaitFuture<'a, E: 'static>  {
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

#[must_use]
pub struct WaitUntilFuture<'a, E: 'static> {
    pub(crate) timeout: Instant,
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

#[must_use]
pub(crate) struct EventReceiverBuilder<'a, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

pub struct EventReceiver<'a, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

#[must_use]
pub struct PollFuture<'a, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

#[must_use]
pub(crate) struct RedrawRequestReceiverBuilder<'a, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

pub struct RedrawRequestReceiver<'a, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

#[must_use]
pub struct RedrawRequestFuture<'a, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

macro_rules! eat_async_events {
    ($shared_state:expr) => {{
        match $shared_state.next_event {
            Some(Event::WindowEvent{..}) |
            Some(Event::DeviceEvent{..}) |
            Some(Event::UserEvent(_)) => {
                $shared_state.next_event = None;
                return Poll::Pending;
            },
            _ => ()
        }
    }};
}

impl<E> Future for WaitFuture<'_, E> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        let mut shared_state = self.shared_state.borrow_mut();
        if shared_state.eat_async_events {
            eat_async_events!(shared_state);
        }
        match shared_state.next_event {
            Some(Event::NewEvents(_)) => {
                shared_state.next_event = None;
                Poll::Pending
            },
            Some(Event::EventsCleared) |
            None => {
                shared_state.next_event = None;
                unsafe{ *shared_state.control_flow.unwrap().as_mut() = ControlFlow::Wait };
                Poll::Pending
            },
            Some(_) => {
                Poll::Ready(())
            },
        }
    }
}

impl<E> Drop for WaitFuture<'_, E> {
    fn drop(&mut self) {
        let mut shared_state = self.shared_state.borrow_mut();
        unsafe{ *shared_state.control_flow.unwrap().as_mut() = ControlFlow::Poll };
    }
}

impl<E> Future for WaitUntilFuture<'_, E> {
    type Output = WaitCanceledCause;

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<WaitCanceledCause> {
        let mut shared_state = self.shared_state.borrow_mut();
        if shared_state.eat_async_events {
            eat_async_events!(shared_state);
        }
        match shared_state.next_event {
            Some(Event::NewEvents(cause)) => {
                shared_state.next_event = None;
                if let StartCause::ResumeTimeReached{..} = cause {
                    Poll::Ready(WaitCanceledCause::ResumeTimeReached)
                } else {
                    Poll::Pending
                }
            },
            Some(Event::EventsCleared) |
            None => {
                unsafe{ *shared_state.control_flow.unwrap().as_mut() = ControlFlow::WaitUntil(self.timeout) };
                Poll::Pending
            },
            Some(_) => Poll::Ready(WaitCanceledCause::EventsReceived),
        }
    }
}

impl<E> Drop for WaitUntilFuture<'_, E> {
    fn drop(&mut self) {
        let mut shared_state = self.shared_state.borrow_mut();
        unsafe{ *shared_state.control_flow.unwrap().as_mut() = ControlFlow::Poll };
    }
}

impl<'el, E> Future for EventReceiverBuilder<'el, E> {
    type Output = EventReceiver<'el, E>;
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<EventReceiver<'el, E>> {
        let mut shared_state = self.shared_state.borrow_mut();
        if shared_state.eat_async_events {
            eat_async_events!(shared_state);
        }
        if shared_state.eat_redraw_events {
            if let Some(Event::WindowEvent{event: WindowEvent::RedrawRequested, ..}) = shared_state.next_event {
                shared_state.next_event = None;
                return Poll::Pending;
            }
        }

        match shared_state.next_event {
            Some(Event::NewEvents(_)) |
            None => {
                shared_state.next_event = None;
                Poll::Pending
            },
            _ => {
                shared_state.eat_async_events = true;
                shared_state.eat_redraw_events = true;
                Poll::Ready(EventReceiver{ shared_state: self.shared_state })
            }
        }
    }
}

impl<'el, E> EventReceiver<'el, E> {
    pub fn next(&mut self) -> PollFuture<'_, E> {
        PollFuture {
            shared_state: &self.shared_state,
        }
    }

    pub fn redraw_requests(self) -> impl Future<Output=RedrawRequestReceiver<'el, E>> {
        RedrawRequestReceiverBuilder {
            shared_state: &self.shared_state,
        }
    }
}

impl<E> Future for PollFuture<'_, E> {
    type Output = Option<EventAsync<E>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<EventAsync<E>>> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event.take() {
            Some(event) => match event {
                Event::WindowEvent{window_id, event: WindowEvent::RedrawRequested} => {
                    shared_state.next_event = Some(Event::WindowEvent{window_id, event: WindowEvent::RedrawRequested});
                    shared_state.eat_async_events = false;
                    Poll::Ready(None)
                },
                Event::EventsCleared => {
                    shared_state.eat_async_events = false;
                    Poll::Ready(None)
                },
                Event::WindowEvent{window_id, event} => Poll::Ready(Some(EventAsync::WindowEvent{window_id, event})),
                Event::DeviceEvent{device_id, event} => Poll::Ready(Some(EventAsync::DeviceEvent{device_id, event})),
                Event::UserEvent(event) => Poll::Ready(Some(EventAsync::UserEvent(event))),
                Event::Suspended => Poll::Ready(Some(EventAsync::Suspended)),
                Event::Resumed => Poll::Ready(Some(EventAsync::Resumed)),
                Event::NewEvents(_) => Poll::Pending,
                Event::LoopDestroyed => unreachable!(),
            },
            None => Poll::Pending,
        }
    }
}

impl<'el, E> Future for RedrawRequestReceiverBuilder<'el, E> {
    type Output = RedrawRequestReceiver<'el, E>;
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<RedrawRequestReceiver<'el, E>> {
        let mut shared_state = self.shared_state.borrow_mut();
        shared_state.eat_async_events = false;
        match shared_state.next_event {
            Some(Event::EventsCleared) |
            Some(Event::NewEvents(_)) |
            Some(Event::WindowEvent{event: WindowEvent::RedrawRequested, ..}) => {
                Poll::Ready(RedrawRequestReceiver{ shared_state: self.shared_state })
            },
            // Eat all non-redraw events.
            _ => {
                // shared_state.next_event = None;
                Poll::Pending
            }
        }
    }
}

impl<'el, E> RedrawRequestReceiver<'el, E> {
    pub fn next(&mut self) -> RedrawRequestFuture<'_, E> {
        RedrawRequestFuture {
            shared_state: &self.shared_state,
        }
    }
}

impl<'el, E> Future for RedrawRequestFuture<'el, E> {
    type Output = Option<WindowId>;

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<WindowId>> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(Event::EventsCleared) |
            Some(Event::NewEvents(_)) => {
                shared_state.eat_redraw_events = false;
                Poll::Ready(None)
            },
            Some(Event::WindowEvent{window_id, event: WindowEvent::RedrawRequested}) => {
                shared_state.next_event = None;
                Poll::Ready(Some(window_id))
            },
            // Eat all non-redraw events.
            _ => {
                shared_state.next_event = None;
                Poll::Pending
            }
        }
    }
}
