use crate::{
    AsyncDriverEvent,
    SharedState,
    EventAsync,
    WaitCancelledCause,
};
use std::{
    cell::RefCell,
    future::Future,
    pin::Pin,
    rc::Rc,
    task::{Context, Poll},
    time::Instant,
};
use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::ControlFlow,
    window::WindowId,
};

pub(crate) struct PendingWhileNone<T> {
    option: Rc<RefCell<Option<T>>>,
}

impl<T> PendingWhileNone<T> {
    pub fn new(option: Rc<RefCell<Option<T>>>) -> PendingWhileNone<T> {
        PendingWhileNone { option }
    }
}

impl<T> Future for PendingWhileNone<T> {
    type Output = T;

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<T> {
        match self.option.borrow_mut().take() {
            Some(t) => Poll::Ready(t),
            None => Poll::Pending,
        }
    }
}

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
    pub(crate) dispatch_redraws: bool,
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

impl<E> Future for WaitFuture<'_, E> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(AsyncDriverEvent::WakeUp(_)) => {
                unsafe{ *shared_state.control_flow.unwrap().as_mut() = ControlFlow::Poll };
                Poll::Ready(())
            },

            Some(AsyncDriverEvent::Event(_)) |
            Some(AsyncDriverEvent::EventsCleared) |
            Some(AsyncDriverEvent::Redraw(_)) |
            Some(AsyncDriverEvent::RedrawsCleared) => {
                shared_state.next_event = None;
                Poll::Pending
            },
            Some(AsyncDriverEvent::SetWait) => {
                shared_state.next_event = None;
                unsafe{ *shared_state.control_flow.unwrap().as_mut() = ControlFlow::Wait };
                Poll::Pending
            },
            None => Poll::Pending,
        }
    }
}

impl<E> Future for WaitUntilFuture<'_, E> {
    type Output = WaitCancelledCause;

    fn poll(mut self: Pin<&mut Self>, _: &mut Context) -> Poll<WaitCancelledCause> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(AsyncDriverEvent::WakeUp(cause)) => {
                unsafe{ *shared_state.control_flow.unwrap().as_mut() = ControlFlow::Poll };
                match cause {
                    StartCause::ResumeTimeReached{..} => Poll::Ready(WaitCancelledCause::ResumeTimeReached),
                    StartCause::WaitCancelled{..} |
                    StartCause::Init => Poll::Ready(WaitCancelledCause::EventsReceived),
                    StartCause::Poll => {
                        shared_state.next_event = None;
                        Poll::Pending
                    }
                }
            },

            Some(AsyncDriverEvent::Event(_)) |
            Some(AsyncDriverEvent::Redraw(_)) |
            Some(AsyncDriverEvent::EventsCleared) |
            Some(AsyncDriverEvent::RedrawsCleared) => {
                shared_state.next_event = None;
                Poll::Pending
            },
            Some(AsyncDriverEvent::SetWait) => {
                shared_state.next_event = None;
                unsafe{ *shared_state.control_flow.unwrap().as_mut() = ControlFlow::WaitUntil(self.timeout) };
                Poll::Pending
            },
            None => Poll::Pending,
        }
    }
}

impl<'el, E> Future for EventReceiverBuilder<'el, E> {
    type Output = EventReceiver<'el, E>;
    fn poll(mut self: Pin<&mut Self>, _: &mut Context) -> Poll<EventReceiver<'el, E>> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(AsyncDriverEvent::SetWait) |
            Some(AsyncDriverEvent::WakeUp(_)) => {
                self.dispatch_redraws = true;
                shared_state.next_event = None;
                Poll::Ready(EventReceiver{ shared_state: self.shared_state })
            },

            Some(AsyncDriverEvent::Redraw(_)) if self.dispatch_redraws => {
                Poll::Ready(EventReceiver{ shared_state: self.shared_state })
            }
            Some(AsyncDriverEvent::EventsCleared) |
            Some(AsyncDriverEvent::Event(_)) |
            Some(AsyncDriverEvent::RedrawsCleared) |
            Some(AsyncDriverEvent::Redraw(_)) => {
                shared_state.next_event = None;
                Poll::Pending
            },
            None => Poll::Pending
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
            Some(AsyncDriverEvent::Event(event)) => Poll::Ready(Some(event)),
            Some(AsyncDriverEvent::WakeUp(_)) => Poll::Ready(None),
            Some(AsyncDriverEvent::SetWait) |
            Some(AsyncDriverEvent::EventsCleared) => Poll::Ready(None),

            Some(AsyncDriverEvent::Redraw(_)) |
            Some(AsyncDriverEvent::RedrawsCleared) => unreachable!(),
            None => Poll::Pending,
        }
    }
}

impl<'el, E> Future for RedrawRequestReceiverBuilder<'el, E> {
    type Output = RedrawRequestReceiver<'el, E>;
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<RedrawRequestReceiver<'el, E>> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(AsyncDriverEvent::RedrawsCleared) |
            Some(AsyncDriverEvent::Redraw(_)) => {
                Poll::Ready(RedrawRequestReceiver {
                    shared_state: self.shared_state,
                })
            },

            Some(AsyncDriverEvent::SetWait) |
            Some(AsyncDriverEvent::WakeUp(_)) |
            Some(AsyncDriverEvent::Event(_)) |
            Some(AsyncDriverEvent::EventsCleared) => {
                shared_state.next_event = None;
                Poll::Pending
            },
            None => Poll::Pending,
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
            Some(AsyncDriverEvent::SetWait) |
            Some(AsyncDriverEvent::WakeUp(_)) |
            Some(AsyncDriverEvent::RedrawsCleared) => {
                shared_state.next_event = None;
                Poll::Ready(None)
            }
            Some(AsyncDriverEvent::Redraw(window_id)) => {
                shared_state.next_event = None;
                Poll::Ready(Some(window_id))
            },

            Some(AsyncDriverEvent::Event(_)) |
            Some(AsyncDriverEvent::EventsCleared) => unreachable!(),
            None => Poll::Pending,
        }
    }
}
