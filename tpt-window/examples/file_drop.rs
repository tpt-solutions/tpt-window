// SPDX-License-Identifier: MIT OR Apache-2.0
//! Prints the paths of files dropped on the window. A path arrives in chunks, so it is rebuilt
//! with `PathAssembler`.
//!
//! `cargo run -p tpt-window --example file_drop`
//!
//! Drag a file from Explorer onto the window. Set `TPT_EXAMPLE_SECONDS=20` to auto-exit.
//! Hovering is reported on the web and on platforms with full drag-and-drop support; on Windows
//! only the drop itself is reported for now.

use std::time::{Duration, Instant};

use tpt_window::PathAssembler;
use tpt_window::prelude::*;

fn main() -> Result<(), WindowError> {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let started = Instant::now();

    let mut event_loop = new_event_loop()?;
    let _window = WindowBuilder::new()
        .with_title("tpt-window: drop files here")
        .build(&event_loop)?;
    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    let mut path = PathAssembler::new();

    loop {
        let status = event_loop.step(&mut EventSink::new(&mut queue), Duration::from_millis(50));
        for ev in queue.drain() {
            match ev.event {
                WindowEvent::FileDropped { chunk, continued } => {
                    if let Some(whole) = path.push(&chunk, continued) {
                        println!("dropped: {whole}");
                    }
                }
                WindowEvent::FileHovered { .. } | WindowEvent::FileHoverCancelled => {}
                WindowEvent::CloseRequested => return Ok(()),
                _ => {}
            }
        }
        if status == PollStatus::Exit || limit.is_some_and(|l| started.elapsed() >= l) {
            break;
        }
    }
    Ok(())
}
