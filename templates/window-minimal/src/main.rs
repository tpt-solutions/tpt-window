use tpt_window::prelude::*;

fn main() -> Result<(), WindowError> {
    // `run` owns the loop: it returns when the window's close button is pressed.
    run(
        WindowBuilder::new().with_title("{{project-name}}"),
        |_window, ev| {
            if ev.event == WindowEvent::CloseRequested {
                Flow::Exit
            } else {
                Flow::Continue
            }
        },
    )
}
