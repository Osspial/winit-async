use crate::{
    SharedState,
    EventAsync,
    WaitCanceledCause,
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

impl<T> PendingWhileNone<T>

impl<T> Future for PendingWhileNone<T> {
    type Output = T;

    fn poll(&mut self, _: &mut Context) -> Poll<T> {
        match self.option.borrow_mut().take {
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
    }
}

impl<'el, E> Future for RedrawRequestReceiverBuilder<'el, E> {
    type Output = RedrawRequestReceiver<'el, E>;
    fn poll(self: Pin<&mut Self>, _: &mut Context) -> Poll<RedrawRequestReceiver<'el, E>> {
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
    }
}
