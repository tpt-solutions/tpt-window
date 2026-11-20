// SPDX-License-Identifier: MIT OR Apache-2.0
//! Keeps the set of held keys and mouse buttons, printing it whenever it changes.
//!
//! The window layer reports presses and releases as events, not as a state you can query.
//! This is the pattern most applications need: fold the events into a set. Focus loss
//! releases everything (the backends synthesize the releases), so the set empties when you
//! alt-tab away with a key held.
//!
//! Set `TPT_EXAMPLE_SECONDS=10` to auto-exit.

use std::collections::BTreeSet;
use std::time::{Duration, Instant};

use tpt_window::*;

/// What is currently held. Names are kept as strings in ordered sets, so the output is stable
/// and readable.
#[derive(Default)]
struct Held {
    keys: BTreeSet<String>,
    buttons: BTreeSet<String>,
}

impl Held {
    /// Applies one event. Returns true when the held set changed.
    fn apply(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                state,
                key_code,
                physical_key: _,
            } => update(&mut self.keys, format!("{key_code:?}"), *state),
            WindowEvent::MouseInput { state, button } => {
                update(&mut self.buttons, format!("{button:?}"), *state)
            }
            _ => false,
        }
    }
}

fn update(set: &mut BTreeSet<String>, name: String, state: ElementState) -> bool {
    match state {
        // A repeated press of a held key is already in the set, so it changes nothing.
        ElementState::Pressed => set.insert(name),
        ElementState::Released => set.remove(&name),
    }
}

fn main() -> Result<(), WindowError> {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let started = Instant::now();

    let mut event_loop = new_event_loop()?;
    let _window = WindowBuilder::new()
        .with_title("tpt-window: input state")
        .build(&event_loop)?;
    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    let mut held = Held::default();

    loop {
        let status = event_loop.step(&mut EventSink::new(&mut queue), Duration::from_millis(50));
        for ev in queue.drain() {
            if held.apply(&ev.event) {
                println!("keys {:?}  buttons {:?}", held.keys, held.buttons);
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
