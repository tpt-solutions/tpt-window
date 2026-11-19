// SPDX-License-Identifier: MIT OR Apache-2.0
//! Window lifecycle on a real X server. Skips when `DISPLAY` is unset, so it runs in
//! CI only on a job that starts one (Xvfb).
#![cfg(target_os = "linux")]

use raw_window_handle::RawWindowHandle;
use tpt_window_core::*;
use tpt_window_sys_x11::new_event_loop;

fn pump(el: &mut EventLoop, q: &mut EventQueue) -> Vec<WindowEvent> {
    el.poll_events(&mut EventSink::new(q));
    q.drain().map(|e| e.event).collect()
}

#[test]
fn window_lifecycle() {
    if std::env::var_os("DISPLAY").is_none() {
        eprintln!("skipping: no DISPLAY");
        return;
    }
    let mut el = new_event_loop().expect("open the X display");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window x11 test")
        .with_inner_size(LogicalSize::new(400.0, 300.0))
        .build(&el)
        .expect("create window");

    assert!(
        matches!(w.raw_window_handle(), Ok(RawWindowHandle::Xlib(_))),
        "expected an Xlib handle"
    );
    pump(&mut el, &mut q);

    // Resizing is a command; X reports the new size as a ConfigureNotify.
    w.set_size(PhysicalSize::new(500, 320));
    let mut resized = false;
    for _ in 0..50 {
        if pump(&mut el, &mut q)
            .iter()
            .any(|e| *e == WindowEvent::Resized(PhysicalSize::new(500, 320)))
        {
            resized = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(resized, "no Resized after set_size");
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
