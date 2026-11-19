// SPDX-License-Identifier: MIT OR Apache-2.0
//! Prints every event except per-frame yield points. Useful for checking a backend.
//!
//! Set `TPT_EXAMPLE_SECONDS=5` to auto-exit.

use std::time::{Duration, Instant};

use tpt_window::*;

fn main() -> Result<(), WindowError> {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let started = Instant::now();

    let mut event_loop = new_event_loop()?;
    let _window = WindowBuilder::new()
        .with_title("tpt-window: event dump")
        .build(&event_loop)?;
    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);

    loop {
        let status = event_loop.step(&mut EventSink::new(&mut queue), Duration::from_millis(50));
        for ev in queue.drain() {
            match ev.event {
                WindowEvent::YieldPoint { .. } => {}
                WindowEvent::CursorMoved { .. } => {} // too noisy
                other => println!("[{:?}] {other:?}", ev.window),
            }
            if ev.event == WindowEvent::CloseRequested {
                event_loop.request_exit();
            }
        }
        if status == PollStatus::Exit || limit.is_some_and(|l| started.elapsed() >= l) {
            break;
        }
    }
    println!("dropped events: {}", queue.dropped());
    Ok(())
}
