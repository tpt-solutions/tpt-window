// SPDX-License-Identifier: MIT OR Apache-2.0
//! A window-level test that needs no display: script the events, run the loop frame by frame,
//! and check what the application did. This is the pattern for testing app logic in CI.
//!
//! `cargo run -p tpt-window-test-utils --example headless_test`

use tpt_window_core::{EventQueue, EventSink, PhysicalSize, WindowBuilder, WindowEvent};
use tpt_window_test_utils::{headless, scenarios};

/// The application logic under test: it closes itself after a resize larger than 1000 pixels.
fn app_step(window: &tpt_window_core::Window, events: &[WindowEvent]) -> bool {
    for event in events {
        if let WindowEvent::Resized(size) = event {
            if size.width > 1000 {
                window.set_visible(false);
                return true;
            }
        }
    }
    false
}

fn main() {
    let (mut event_loop, handle) = headless();
    event_loop.set_emit_yield_points(false);
    let window = WindowBuilder::new().build(&event_loop).expect("window");

    // Frame 1: a small resize, which the app ignores.
    handle.inject_at(
        1,
        window.id(),
        WindowEvent::Resized(PhysicalSize::new(640, 480)),
    );
    // Frame 2: a large resize, which makes the app hide its window.
    scenarios::dpi_change(&handle, 2, window.id(), 1.0, PhysicalSize::new(1280, 720));
    handle.inject_at(
        2,
        window.id(),
        WindowEvent::Resized(PhysicalSize::new(1280, 720)),
    );

    let mut queue = EventQueue::with_capacity(16);
    let mut closed = false;
    // Four frames: the window command an app sends in one frame applies on the next poll.
    for frame in 0..4 {
        event_loop.poll_events(&mut EventSink::new(&mut queue));
        let events: Vec<WindowEvent> = queue.drain().map(|e| e.event).collect();
        println!("frame {frame}: {events:?}");
        closed |= app_step(&window, &events);
    }

    assert!(closed, "the app should have reacted to the large resize");
    assert!(!window.is_visible(), "the app hid its window");
    println!("headless test passed");
}
