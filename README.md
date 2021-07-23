# winit-async

Experimental asynchronous event loop for winit, for use with any async framework.

## Usage

Copied from the example:

```rust
pub use winit::{
    winit::{
        dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
        event::WindowEvent,
        event_loop::EventLoop,
        window::WindowBuilder,
    },
    EventAsync as Event,
    EventLoopAsync,
    HasRawWindowHandle,
    RawWindowHandle,
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    
    window.request_redraw();
    
    event_loop.run_async(move |mut runner| async move {
        'main: loop {
            runner.wait().await;
            
            let mut recv_events = runner.recv_events().await;
            while let Some(event) = recv_events.next().await {
                if let Event::WindowEvent {
                    event: WindowEvent::CloseRequested, ..
                } = event_async {
                    break 'main;
                }
            }//end of event-loop
            
            let mut redraw_requests = recv_events.redraw_requests().await;
            while let Some(window_id) = redraw_requests.next().await {
                println!("redraw {:?}", window_id);
                window.request_redraw();
            }
            
        }//end of main-loop
        
        drop(window);
        drop(runner);
    })
}
```

## Usage with Tokio

```rust
pub use tokio;

pub use winit::{
    winit::{
        dpi::{LogicalSize, PhysicalPosition, PhysicalSize},
        event::WindowEvent,
        event_loop::EventLoop,
        window::WindowBuilder,
    },
    EventAsync as Event,
    EventLoopAsync,
    HasRawWindowHandle,
    RawWindowHandle,
};

fn main() {
    let runtime = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    let _guard = runtime.enter();
    
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();
    
    window.request_redraw();
    
    // Construct and enter a local task-set.
    let event_loop_local_set = tokio::task::LocalSet::new();
    event_loop_local_set.run_until(async move {
        // Outer main loop
        event_loop.run_async(move |mut runner| async move {
            // Inner main loop
            'main: loop {
                runner.wait().await;
                
                let mut recv_events = runner.recv_events().await;
                while let Some(event) = recv_events.next().await {
                    if let Event::WindowEvent {
                        event: WindowEvent::CloseRequested, ..
                    } = event_async {
                        break 'main;
                    }
                }//end of event-loop
                
                let mut redraw_requests = recv_events.redraw_requests().await;
                while let Some(window_id) = redraw_requests.next().await {
                    println!("redraw {:?}", window_id);
                    window.request_redraw();
                }
                
            }//end of main-loop
            
            drop(window);
            drop(runner);
        })
    });
}
```
