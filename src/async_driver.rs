use std::future::Future;
use winit::{
    event::{Event, StartCause, WindowEvent},
    window::WindowId,
};
use crate::EventAsync;

pub trait EventSink<'a> {
    type Event: 'static;
    #[must_use]
    fn dispatch_event(&'a mut self, event: EventAsync<Self::Event>) -> bool;
    #[must_use]
    fn events_cleared(&'a mut self) -> bool;
    #[must_use]
    fn dispatch_redraw(&'a mut self, window: WindowId) -> bool;
    #[must_use]
    fn redraws_cleared(&'a mut self) -> bool;
    #[must_use]
    fn set_wait(&'a mut self) -> bool;
    #[must_use]
    fn wake_up(&'a mut self, start_cause: StartCause) -> bool;
}

pub async fn async_driver<E: std::fmt::Debug, EFn, EFu, ESk>(
    event_source: EFn,
    mut event_sink: ESk,
)
    where E: 'static,
          EFn: FnMut() -> EFu,
          EFu: Future<Output=Event<E>>,
          ESk: for<'a> EventSink<'a, Event=E>,
{
    let mut next_event = event_source;

    macro_rules! return_if_done {
        ($sink_call:expr) => {{
            match $sink_call {
                true => return,
                false => (),
            }
        }};
    }

    let mut break_event = None;
    loop {
        break_event = loop {
            let event =  match break_event.take() {
                Some(event) => event,
                None => next_event().await
            };

            match event {
                Event::EventsCleared => return_if_done!(event_sink.set_wait()),
                Event::NewEvents(cause) => return_if_done!(event_sink.wake_up(cause)),
                _ => break Some(event),
            }
        };

        break_event = loop {
            let event =  match break_event.take() {
                Some(event) => event,
                None => next_event().await
            };
            match event {
                Event::WindowEvent{window_id: _, event: WindowEvent::RedrawRequested} |
                Event::EventsCleared => break Some(event),
                Event::LoopDestroyed => return,

                Event::WindowEvent{window_id, event} => return_if_done!(event_sink.dispatch_event(EventAsync::WindowEvent{window_id, event})),
                Event::DeviceEvent{device_id, event} => return_if_done!(event_sink.dispatch_event(EventAsync::DeviceEvent{device_id, event})),
                Event::UserEvent(event) => return_if_done!(event_sink.dispatch_event(EventAsync::UserEvent(event))),
                Event::Suspended => return_if_done!(event_sink.dispatch_event(EventAsync::Suspended)),
                Event::Resumed => return_if_done!(event_sink.dispatch_event(EventAsync::Resumed)),
                Event::NewEvents(_) => unreachable!(),
            }
        };

        return_if_done!(event_sink.events_cleared());

        break_event = loop {
            let event = match break_event.take() {
                Some(event) => event,
                None => next_event().await
            };

            match event {
                Event::WindowEvent{window_id, event: WindowEvent::RedrawRequested} => {
                    return_if_done!(event_sink.dispatch_redraw(window_id))
                },
                Event::EventsCleared => (),
                Event::NewEvents(_) => break Some(event),
                Event::LoopDestroyed => return,

                Event::WindowEvent{..} |
                Event::DeviceEvent{..} |
                Event::UserEvent(..) |
                Event::Suspended |
                Event::Resumed => unreachable!()
            }
        };

        return_if_done!(event_sink.redraws_cleared());
    }
}
