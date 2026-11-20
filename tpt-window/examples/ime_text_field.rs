// SPDX-License-Identifier: MIT OR Apache-2.0
//! A text field that takes input from an input method. Composition (Preedit) is shown underlined
//! in a real UI; here it is printed, and the committed text is appended to the field.
//!
//! The candidate list follows the caret: `set_ime_cursor_area` tells the input method where the
//! caret is, in physical pixels relative to the window.
//!
//! `cargo run -p tpt-window --example ime_text_field`
//!
//! Set `TPT_EXAMPLE_SECONDS=20` to auto-exit. Input methods are not installed on every machine;
//! without one, typed text still arrives as committed text.

use std::time::{Duration, Instant};

use tpt_window::prelude::*;
use tpt_window::{ImeEvent, PhysicalPosition};

/// The text field's state.
#[derive(Default)]
struct Field {
    committed: String,
    composing: String,
}

impl Field {
    fn show(&self) {
        println!("field: {}[{}]", self.committed, self.composing);
    }
}

fn main() -> Result<(), WindowError> {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let started = Instant::now();

    let mut event_loop = new_event_loop()?;
    let window = WindowBuilder::new()
        .with_title("tpt-window: text field")
        .build(&event_loop)?;
    // The caret sits 40 pixels down and 20 across. The candidate list opens below it.
    window.set_ime_cursor_area(PhysicalPosition::new(20, 40), PhysicalSize::new(2, 24));

    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    let mut field = Field::default();
    // A text input's text can be longer than one event carries; long pieces arrive in chunks,
    // so the chunks are joined before they are used.
    let mut pending_commit = String::new();

    loop {
        let status = event_loop.step(&mut EventSink::new(&mut queue), Duration::from_millis(50));
        for ev in queue.drain() {
            match ev.event {
                WindowEvent::Ime(ImeEvent::Preedit { text, continued }) => {
                    // Each preedit replaces the previous one, so only the last chunk set is kept.
                    if !continued {
                        field.composing = text.as_str().to_owned();
                        field.show();
                    }
                }
                WindowEvent::Ime(ImeEvent::Commit { text, continued }) => {
                    pending_commit.push_str(text.as_str());
                    if !continued {
                        field
                            .committed
                            .push_str(&std::mem::take(&mut pending_commit));
                        field.composing.clear();
                        field.show();
                    }
                }
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
