use crate::{EventAsync, SharedState, WaitCanceledCause};
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
    window::WindowId,
};

#[must_use]
pub struct WaitFuture<'a, 'e, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<'e, E>>,
}

#[must_use]
pub struct WaitUntilFuture<'a, 'e, E: 'static> {
    pub(crate) timeout: Instant,
    pub(crate) shared_state: &'a RefCell<SharedState<'e, E>>,
}

#[must_use]
pub(crate) struct EventReceiverBuilder<'a, 'e, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<'e, E>>,
}

pub struct EventReceiver<'a, 'e, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<'e, E>>,
}

#[must_use]
pub struct PollFuture<'a, 'e, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<'e, E>>,
    pub(crate) sealed: bool,
}

#[must_use]
pub(crate) struct RedrawRequestReceiverBuilder<'a, 'e, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<'e, E>>,
}

pub struct RedrawRequestReceiver<'a, 'e, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<'e, E>>,
}

#[must_use]
pub struct RedrawRequestFuture<'a, 'e, E: 'static> {
    pub(crate) shared_state: &'a RefCell<SharedState<'e, E>>,
    pub(crate) sealed: bool,
}

impl<E> Future for WaitFuture<'_, '_, E> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<()> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(Event::NewEvents { .. }) => {
                unsafe { *shared_state.control_flow.unwrap().as_mut() = ControlFlow::Poll };
                Poll::Ready(())
            }
            Some(Event::RedrawEventsCleared) => {
                unsafe { *shared_state.control_flow.unwrap().as_mut() = ControlFlow::Wait };
                shared_state.next_event = None;
                Poll::Pending
            }
            Some(Event::WindowEvent { .. })
            | Some(Event::DeviceEvent { .. })
            | Some(Event::UserEvent { .. })
            | Some(Event::MainEventsCleared)
            | Some(Event::RedrawRequested { .. }) => {
                shared_state.next_event = None;
                Poll::Pending
            }
            Some(Event::LoopDestroyed) => unreachable!(),
            Some(Event::Suspended) | Some(Event::Resumed) => unimplemented!(),
            None => Poll::Pending,
        }
    }
}

impl<E> Future for WaitUntilFuture<'_, '_, E> {
    type Output = WaitCanceledCause;

    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<WaitCanceledCause> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(Event::NewEvents(cause)) => {
                unsafe { *shared_state.control_flow.unwrap().as_mut() = ControlFlow::Poll };
                Poll::Ready(match cause {
                    StartCause::ResumeTimeReached { .. } => WaitCanceledCause::ResumeTimeReached,
                    StartCause::WaitCancelled { .. } | StartCause::Poll | StartCause::Init => {
                        WaitCanceledCause::EventsReceived
                    }
                })
            }
            Some(Event::RedrawEventsCleared) => {
                unsafe {
                    *shared_state.control_flow.unwrap().as_mut() =
                        ControlFlow::WaitUntil(self.timeout)
                };
                shared_state.next_event = None;
                Poll::Pending
            }
            Some(Event::WindowEvent { .. })
            | Some(Event::DeviceEvent { .. })
            | Some(Event::UserEvent { .. })
            | Some(Event::MainEventsCleared)
            | Some(Event::RedrawRequested { .. }) => {
                shared_state.next_event = None;
                Poll::Pending
            }
            Some(Event::LoopDestroyed) => unreachable!(),
            Some(Event::Suspended) | Some(Event::Resumed) => unimplemented!(),
            None => Poll::Pending,
        }
    }
}

impl<'el, 'ev, E> Future for EventReceiverBuilder<'el, 'ev, E> {
    type Output = EventReceiver<'el, 'ev, E>;
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<EventReceiver<'el, 'ev, E>> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(Event::RedrawRequested { .. })
            | Some(Event::RedrawEventsCleared)
            | Some(Event::MainEventsCleared) => {
                shared_state.next_event = None;
                Poll::Pending
            }
            Some(Event::NewEvents(_)) => {
                shared_state.next_event = None;
                Poll::Ready(EventReceiver {
                    shared_state: self.shared_state,
                })
            }
            Some(Event::WindowEvent { .. })
            | Some(Event::DeviceEvent { .. })
            | Some(Event::UserEvent { .. })
            | Some(Event::LoopDestroyed) => unreachable!(),
            Some(Event::Suspended) | Some(Event::Resumed) => unimplemented!(),
            None => Poll::Pending,
        }
    }
}

impl<'el, 'ev, E> EventReceiver<'el, 'ev, E> {
    pub fn next(&mut self) -> PollFuture<'_, 'ev, E> {
        PollFuture {
            shared_state: &self.shared_state,
            sealed: false,
        }
    }

    pub fn redraw_requests(self) -> impl Future<Output = RedrawRequestReceiver<'el, 'ev, E>> {
        RedrawRequestReceiverBuilder {
            shared_state: &self.shared_state,
        }
    }
}

impl<'a, 'ev, E> Future for PollFuture<'a, 'ev, E> {
    type Output = Option<EventAsync<'ev, E>>;

    fn poll(mut self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<EventAsync<'ev, E>>> {
        if self.sealed {
            return Poll::Ready(None);
        }

        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event.take() {
            Some(Event::WindowEvent { window_id, event }) => {
                Poll::Ready(Some(EventAsync::WindowEvent { window_id, event }))
            }
            Some(Event::DeviceEvent { device_id, event }) => {
                Poll::Ready(Some(EventAsync::DeviceEvent { device_id, event }))
            }
            Some(Event::UserEvent(event)) => Poll::Ready(Some(EventAsync::UserEvent(event))),
            Some(Event::MainEventsCleared) => {
                self.sealed = true;
                Poll::Ready(None)
            }
            event @ Some(Event::RedrawRequested { .. })
            | event @ Some(Event::RedrawEventsCleared) => {
                shared_state.next_event = event;
                Poll::Ready(None)
            }
            Some(Event::NewEvents(_)) | Some(Event::LoopDestroyed) => unreachable!(),
            Some(Event::Suspended) | Some(Event::Resumed) => unimplemented!(),
            None => Poll::Pending,
        }
    }
}

impl<'el, 'ev, E> Future for RedrawRequestReceiverBuilder<'el, 'ev, E> {
    type Output = RedrawRequestReceiver<'el, 'ev, E>;
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<RedrawRequestReceiver<'el, 'ev, E>> {
        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(Event::RedrawRequested { .. }) | Some(Event::RedrawEventsCleared) => {
                Poll::Ready(RedrawRequestReceiver {
                    shared_state: self.shared_state,
                })
            }
            Some(Event::MainEventsCleared)
            | Some(Event::WindowEvent { .. })
            | Some(Event::DeviceEvent { .. })
            | Some(Event::UserEvent { .. }) => {
                shared_state.next_event = None;
                Poll::Pending
            }
            Some(Event::NewEvents { .. }) | Some(Event::LoopDestroyed) => unreachable!(),
            Some(Event::Suspended) | Some(Event::Resumed) => unimplemented!(),
            None => Poll::Pending,
        }
    }
}

impl<'el, 'ev, E> RedrawRequestReceiver<'el, 'ev, E> {
    pub fn next(&mut self) -> RedrawRequestFuture<'_, 'ev, E> {
        RedrawRequestFuture {
            shared_state: &self.shared_state,
            sealed: false,
        }
    }
}

impl<'el, 'ev, E> Future for RedrawRequestFuture<'el, 'ev, E> {
    type Output = Option<WindowId>;

    fn poll(mut self: Pin<&mut Self>, _: &mut Context) -> Poll<Option<WindowId>> {
        if self.sealed {
            return Poll::Ready(None);
        }

        let mut shared_state = self.shared_state.borrow_mut();
        match shared_state.next_event {
            Some(Event::RedrawRequested(window_id)) => {
                shared_state.next_event = None;
                Poll::Ready(Some(window_id))
            }
            Some(Event::RedrawEventsCleared) => {
                self.sealed = true;
                Poll::Ready(None)
            }
            Some(Event::WindowEvent { .. })
            | Some(Event::DeviceEvent { .. })
            | Some(Event::UserEvent { .. })
            | Some(Event::MainEventsCleared) => {
                shared_state.next_event = None;
                Poll::Pending
            }

            Some(Event::NewEvents { .. }) | Some(Event::LoopDestroyed) => unreachable!(),
            Some(Event::Suspended) | Some(Event::Resumed) => unimplemented!(),
            None => Poll::Pending,
        }
    }
}
