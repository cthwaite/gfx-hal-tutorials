extern crate winit;

use winit::{Event, EventsLoop, KeyboardInput, VirtualKeyCode, WindowBuilder, WindowEvent};


fn main() {
    let mut events_loop = EventsLoop::new();
    let window = WindowBuilder::new()
                    .with_title("Part 00: Triangle")
                    .with_dimensions((256, 256).into())
                    .with_decorations(false)
                    .build(&events_loop)
                    .unwrap();

    loop {
        let mut quitting = false;
        events_loop.poll_events(|event| {
            if let Event::WindowEvent { event, .. } = event {
                match event {
                    WindowEvent::CloseRequested => quitting = true,
                    WindowEvent::KeyboardInput {
                        input: KeyboardInput {
                            virtual_keycode: Some(VirtualKeyCode::Escape),
                            ..
                        },
                        ..
                    } => quitting = true,
                    _ => ()
                }

            }
        });

        if quitting {
            break;
        }
    }
}
