use std::future::Future;
use winit::{
    event::{Event, StartCause, WindowEvent},
    window::WindowId,
};
use crate::EventAsync;

pub trait EventSink<'a> {
    type Event: 'static;
    type DispatchEvent: 'a + Future<Output=()>;
    type DispatchRedraw: 'a + Future<Output=()>;
    type TryWait: 'a + Future<Output=()>;
    fn dispatch_event(&'a mut self, event: EventAsync<Self::Event>) -> Self::DispatchEvent;
    fn dispatch_redraw(&'a mut self, window: WindowId) -> Self::DispatchRedraw;
    fn try_wait(&'a mut self) -> Self::TryWait;
}

pub async fn async_driver<E, EFn, EFu, ESk>(
    event_source: EFn,
    mut event_sink: ESk,
)
    where E: 'static,
          EFn: FnMut() -> EFu,
          EFu: Future<Output=Event<E>>,
          ESk: for<'a> EventSink<'a, Event=E>,
{
    let mut next_event = event_source;
    match next_event().await {
        Event::NewEvents(StartCause::Init) => (),
        _ => panic!("Unexpected first event")
    }

    loop {
        let poll_break_event = loop {
            let event = next_event().await;
            match event {
                Event::WindowEvent{window_id: _, event: WindowEvent::RedrawRequested} |
                Event::EventsCleared => break event,
                Event::LoopDestroyed => return,

                Event::WindowEvent{window_id, event} => event_sink.dispatch_event(EventAsync::WindowEvent{window_id, event}).await,
                Event::DeviceEvent{device_id, event} => event_sink.dispatch_event(EventAsync::DeviceEvent{device_id, event}).await,
                Event::UserEvent(event) => event_sink.dispatch_event(EventAsync::UserEvent(event)).await,
                Event::Suspended => event_sink.dispatch_event(EventAsync::Suspended).await,
                Event::Resumed => event_sink.dispatch_event(EventAsync::Resumed).await,
                Event::NewEvents(_) => unreachable!(),
            }
        };

        let mut poll_break_event_opt = Some(poll_break_event);
        loop {
            let event = match poll_break_event_opt.take() {
                Some(event) => event,
                None => next_event().await
            };

            match event {
                Event::WindowEvent{window_id, event: WindowEvent::RedrawRequested} => event_sink.dispatch_redraw(window_id).await,
                Event::EventsCleared => (),
                Event::NewEvents(_) => break,
                Event::LoopDestroyed => return,

                Event::WindowEvent{..} |
                Event::DeviceEvent{..} |
                Event::UserEvent(..) |
                Event::Suspended |
                Event::Resumed => unreachable!()
            }
        }

        event_sink.try_wait().await;
    }
}
