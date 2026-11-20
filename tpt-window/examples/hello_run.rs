// SPDX-License-Identifier: MIT OR Apache-2.0
//! The same window as `hello_window`, with the loop written by `run`. Closes on the close
//! button, or after `TPT_EXAMPLE_SECONDS` seconds.

use std::time::{Duration, Instant};

use tpt_window::prelude::*;

fn main() -> Result<(), WindowError> {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let started = Instant::now();

    run(
        WindowBuilder::new().with_title("tpt-window: hello run"),
        |_window, ev| {
            if ev.event == WindowEvent::CloseRequested
                || limit.is_some_and(|l| started.elapsed() >= l)
            {
                Flow::Exit
            } else {
                Flow::Continue
            }
        },
    )
}
