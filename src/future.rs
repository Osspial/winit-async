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
    event::{Event, StartCause},
    event_loop::ControlFlow,
};

#[must_use]
pub struct WaitFuture<'a, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

#[must_use]
pub struct WaitUntilFuture<'a, E: 'static> {
    pub(crate) timeout: Instant,
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

#[must_use]
pub struct PollFuture<'a, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<E>>,
}

impl<E> Future for WaitFuture<'_, E> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        let mut shared_state = self.shared_state.borrow_mut();
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

impl<E> Future for PollFuture<'_, E> {
    type Output = Option<EventAsync<E>>;

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<EventAsync<E>>> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event.take() {
            Some(event) => match event {
                Event::WindowEvent{window_id, event} => Poll::Ready(Some(EventAsync::WindowEvent{window_id, event})),
                Event::DeviceEvent{device_id, event} => Poll::Ready(Some(EventAsync::DeviceEvent{device_id, event})),
                Event::UserEvent(event) => Poll::Ready(Some(EventAsync::UserEvent(event))),
                Event::EventsCleared => Poll::Ready(None),
                Event::NewEvents(_) => Poll::Pending,
                Event::Suspended(_) => unimplemented!(),
                Event::LoopDestroyed => unreachable!(),
            },
            None => Poll::Pending,
        }
    }
}
