use tao::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

fn main() {
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().with_title("Test").build(&event_loop).unwrap();

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                println!("Close requested, hiding window");
                window.set_visible(false);
            }
            Event::NewEvents(cause) => println!("New events: {:?}", cause),
            Event::WindowEvent { event: WindowEvent::KeyboardInput { .. }, .. } => {}
            Event::WindowEvent { event: WindowEvent::CursorMoved { .. }, .. } => {}
            Event::MainEventsCleared => {}
            Event::RedrawRequested(_) => {}
            Event::RedrawEventsCleared => {}
            Event::DeviceEvent { .. } => {}
            e => println!("Other Event: {:?}", e),
        }
    });
}
