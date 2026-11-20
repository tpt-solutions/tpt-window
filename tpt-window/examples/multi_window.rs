// SPDX-License-Identifier: MIT OR Apache-2.0
//! Two windows on one event loop. Events carry the window they belong to, so one queue
//! serves both, and the loop exits when every window is closed.
//!
//! Set `TPT_EXAMPLE_SECONDS=5` to auto-exit.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use tpt_window::*;

fn main() -> Result<(), WindowError> {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let started = Instant::now();

    let mut event_loop = new_event_loop()?;
    // Each window lives while its handle is `Some`: dropping the handle closes the window.
    let mut left = Some(
        WindowBuilder::new()
            .with_title("tpt-window: left")
            .with_position(LogicalPosition::new(100.0, 100.0))
            .with_inner_size(LogicalSize::new(320.0, 240.0))
            .build(&event_loop)?,
    );
    let mut right = Some(
        WindowBuilder::new()
            .with_title("tpt-window: right")
            .with_position(LogicalPosition::new(460.0, 100.0))
            .with_inner_size(LogicalSize::new(320.0, 240.0))
            .build(&event_loop)?,
    );
    let (left_id, right_id) = (left.as_ref().unwrap().id(), right.as_ref().unwrap().id());
    let names = [(left_id, "left"), (right_id, "right")];

    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    let mut open: BTreeSet<WindowId> = names.iter().map(|(id, _)| *id).collect();

    while !open.is_empty() {
        let status = event_loop.step(&mut EventSink::new(&mut queue), Duration::from_millis(50));
        for ev in queue.drain() {
            let name = names
                .iter()
                .find(|(id, _)| *id == ev.window)
                .map_or("?", |(_, n)| *n);
            match ev.event {
                WindowEvent::YieldPoint { .. } | WindowEvent::CursorMoved { .. } => {}
                WindowEvent::CloseRequested => {
                    println!("[{name}] close requested");
                    // Dropping the handle destroys the window.
                    if ev.window == left_id {
                        drop(left.take());
                    } else if ev.window == right_id {
                        drop(right.take());
                    }
                    open.remove(&ev.window);
                }
                other => println!("[{name}] {other:?}"),
            }
        }
        if status == PollStatus::Exit || limit.is_some_and(|l| started.elapsed() >= l) {
            break;
        }
    }
    println!("dropped events: {}", queue.dropped());
    Ok(())
}
