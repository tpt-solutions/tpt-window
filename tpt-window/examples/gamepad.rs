// SPDX-License-Identifier: MIT OR Apache-2.0
//! Prints gamepad button presses and stick motion. Gamepads are a separate stream from window
//! events, so this runs without a window. Requires the `gamepad` feature.
//!
//! `cargo run -p tpt-window --example gamepad --features gamepad`
//!
//! Set `TPT_EXAMPLE_SECONDS=10` to auto-exit. With no controller attached it prints nothing.

use std::time::{Duration, Instant};

use tpt_window::gamepad::{GamepadEvent, Gamepads};

fn main() {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64)
        .unwrap_or(Duration::from_secs(5));
    let started = Instant::now();

    let mut pads = match Gamepads::new() {
        Ok(pads) => pads,
        Err(e) => {
            eprintln!("no gamepad backend: {e}");
            return;
        }
    };
    let mut events = Vec::new();
    while started.elapsed() < limit {
        // Never blocks: call it once per frame, reusing the Vec.
        pads.poll(&mut events);
        for ev in events.drain(..) {
            match ev {
                GamepadEvent::Connected(id) => println!("pad {id:?} connected"),
                GamepadEvent::Disconnected(id) => println!("pad {id:?} disconnected"),
                GamepadEvent::Button {
                    id,
                    button,
                    pressed,
                } => println!(
                    "pad {id:?}: {button:?} {}",
                    if pressed { "down" } else { "up" }
                ),
                GamepadEvent::Axis { id, axis, value } => {
                    if value.abs() > 0.2 {
                        println!("pad {id:?}: {axis:?} = {value:+.2}");
                    }
                }
                _ => {}
            }
        }
        std::thread::sleep(Duration::from_millis(16));
    }
}
