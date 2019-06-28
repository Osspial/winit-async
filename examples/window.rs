#![feature(async_await)]
use winit::{
    event::{WindowEvent},
    event_loop::EventLoop,
    window::WindowBuilder,
};
use winit_async::{EventLoopAsync, EventAsync as Event};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    event_loop.run_async(async move |mut runner| {
        loop {
            runner.wait().await;
            println!("loop");

            while let Some(event) = runner.poll().await {
                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => {
                        return;
                    },
                    _ => println!("{:?}", event),
                }
            }
        }
    })
}
