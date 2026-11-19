// SPDX-License-Identifier: MIT OR Apache-2.0
//! Opens a window and runs a caller-owned main loop. Press Escape or close the window.
//!
//! Set `TPT_EXAMPLE_SECONDS=2` to auto-exit (used for smoke tests).

use std::time::{Duration, Instant};

use tpt_window::*;

fn main() -> Result<(), WindowError> {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let started = Instant::now();

    let mut event_loop = new_event_loop()?;
    let window = WindowBuilder::new()
        .with_title("tpt-window: hello")
        .with_inner_size(LogicalSize::new(640.0, 360.0))
        .build(&event_loop)?;
    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    let mut frames = 0u64;

    'main: loop {
        // Block up to 16 ms for input, like a ~60 Hz frame.
        let status = event_loop.step(&mut EventSink::new(&mut queue), Duration::from_millis(16));
        for ev in queue.drain() {
            match ev.event {
                WindowEvent::CloseRequested => break 'main,
                WindowEvent::KeyboardInput {
                    state: ElementState::Pressed,
                    key_code: KeyCode::Escape,
                    ..
                } => break 'main,
                WindowEvent::Resized(s) => println!("resized {}x{}", s.width, s.height),
                WindowEvent::YieldPoint { .. } => frames += 1, // AI agents may run here
                _ => {}
            }
        }
        if status == PollStatus::Exit || limit.is_some_and(|l| started.elapsed() >= l) {
            break;
        }
    }
    println!(
        "bye: {} frames, final size {:?}, scale {}",
        frames,
        window.size(),
        window.scale_factor()
    );
    Ok(())
}
