// SPDX-License-Identifier: MIT OR Apache-2.0
//! Records a session's window events to a file, then replays the file with no window at all.
//!
//! ```text
//! cargo run -p tpt-window --example record_replay -- record session.tptrec 5
//! cargo run -p tpt-window --example record_replay -- replay session.tptrec
//! ```
//!
//! `record <file> [seconds]` opens a window and writes every event to `<file>`, one batch per
//! poll, so the frame boundaries are kept. `replay <file>` runs the same events through the
//! headless backend, frame for frame, and prints them. A replay is deterministic: the same file
//! gives the same events every time, which is what a bug report or a regression test needs.

use std::time::{Duration, Instant};

use tpt_window::prelude::*;
use tpt_window_core::record::{Recorder, read_log};

fn record(path: &str, seconds: f64) -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = new_event_loop()?;
    let _window = WindowBuilder::new()
        .with_title("tpt-window: recording")
        .build(&event_loop)?;
    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    let mut recorder = Recorder::new();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs_f64(seconds) {
        let status = event_loop.step(&mut EventSink::new(&mut queue), Duration::from_millis(16));
        recorder.record_queue(&mut queue);
        if status == PollStatus::Exit {
            break;
        }
    }
    std::fs::write(path, recorder.into_bytes())?;
    println!("wrote {path}");
    Ok(())
}

fn replay(path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(path)?;
    // Check the file first, so a damaged recording is reported as such.
    let events = read_log(&bytes)?;
    println!("{} events in {path}", events.len());
    let (mut event_loop, _handle) = tpt_window_test_utils::replay(&bytes)?;
    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    loop {
        let status = event_loop.poll_events(&mut EventSink::new(&mut queue));
        for ev in queue.drain() {
            if !matches!(ev.event, WindowEvent::YieldPoint { .. }) {
                println!("[{:?}] {:?}", ev.window, ev.event);
            }
        }
        if status == PollStatus::Exit {
            break;
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    match args.as_slice() {
        [_, mode, path] if mode == "replay" => replay(path),
        [_, mode, path] if mode == "record" => record(path, 5.0),
        [_, mode, path, secs] if mode == "record" => record(path, secs.parse()?),
        _ => {
            eprintln!("usage: record_replay record <file> [seconds] | replay <file>");
            Ok(())
        }
    }
}
