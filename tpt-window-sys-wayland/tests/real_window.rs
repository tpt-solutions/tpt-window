// SPDX-License-Identifier: MIT OR Apache-2.0
//! Window lifecycle on a real Wayland compositor. Skips when none is reachable, so
//! it runs in CI only on a job that starts one (for example headless weston).
#![cfg(target_os = "linux")]

use raw_window_handle::RawWindowHandle;
use tpt_window_core::*;
use tpt_window_sys_wayland::new_event_loop;

fn pump(el: &mut EventLoop, q: &mut EventQueue) -> Vec<WindowEvent> {
    el.poll_events(&mut EventSink::new(q));
    q.drain().map(|e| e.event).collect()
}

#[test]
fn window_lifecycle() {
    if std::env::var_os("WAYLAND_DISPLAY").is_none() {
        eprintln!("skipping: no WAYLAND_DISPLAY");
        return;
    }
    let mut el = new_event_loop().expect("connect to the compositor");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window wayland test")
        .with_inner_size(LogicalSize::new(400.0, 300.0))
        .build(&el)
        .expect("create window");

    assert!(
        matches!(w.raw_window_handle(), Ok(RawWindowHandle::Wayland(_))),
        "expected a Wayland handle"
    );

    // The compositor's first configure reports a size; a resize may follow. Wait
    // a few polls for it rather than assuming an event order.
    let mut got_size = false;
    for _ in 0..50 {
        if pump(&mut el, &mut q)
            .iter()
            .any(|e| matches!(e, WindowEvent::Resized(_)))
        {
            got_size = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(got_size, "no configure from the compositor");

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
