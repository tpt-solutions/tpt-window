// SPDX-License-Identifier: MIT OR Apache-2.0
//! Drives a real (hidden) AppKit window. Runs only on macOS, so it executes in CI
//! on the macOS runner.
#![cfg(target_os = "macos")]

use raw_window_handle::RawWindowHandle;
use tpt_window_core::*;
use tpt_window_sys_cocoa::new_event_loop;

fn pump(el: &mut EventLoop, q: &mut EventQueue) -> Vec<WindowEvent> {
    el.poll_events(&mut EventSink::new(q));
    q.drain().map(|e| e.event).collect()
}

#[test]
fn window_lifecycle() {
    let mut el = new_event_loop().expect("event loop on the main thread");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window cocoa test")
        .with_inner_size(LogicalSize::new(400.0, 300.0))
        .with_visible(false)
        .build(&el)
        .expect("create window");

    assert!(
        matches!(w.raw_window_handle(), Ok(RawWindowHandle::AppKit(_))),
        "expected an AppKit handle"
    );
    assert!(!w.is_visible());
    pump(&mut el, &mut q);

    // Commands are applied on the next poll and reported as a resize.
    w.set_size(PhysicalSize::new(500, 320));
    let evs = pump(&mut el, &mut q);
    assert!(
        evs.contains(&WindowEvent::Resized(PhysicalSize::new(500, 320))),
        "got {evs:?}"
    );
    assert_eq!(w.size(), PhysicalSize::new(500, 320));

    // Dropping the last handle destroys the window on the next poll.
    drop(w);
    let evs = pump(&mut el, &mut q);
    assert!(evs.contains(&WindowEvent::Destroyed), "got {evs:?}");

    el.request_exit();
    assert_eq!(
        el.poll_events(&mut EventSink::new(&mut q)),
        PollStatus::Exit
    );
}
