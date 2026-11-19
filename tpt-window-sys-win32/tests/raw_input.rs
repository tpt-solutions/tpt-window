// SPDX-License-Identifier: MIT OR Apache-2.0
//! Relative mouse motion injected with SendInput arrives as raw input on a real
//! window.
#![cfg(windows)]

use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_MOUSE, MOUSEEVENTF_MOVE, MOUSEINPUT, SendInput,
};

#[test]
fn relative_mouse_motion_arrives_as_raw_input() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window raw input test")
        .with_position(LogicalPosition::new(200.0, 200.0))
        .with_inner_size(LogicalSize::new(300.0, 200.0))
        .build(&el)
        .expect("window");
    let _ = &w;

    // A relative move of (20, -10) counts. Raw input reports counts, not screen
    // pixels, so only the direction is checked strictly.
    let mv = INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 20,
                dy: -10,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    };
    let sent = unsafe { SendInput(1, &mv, core::mem::size_of::<INPUT>() as i32) };
    assert_eq!(sent, 1, "SendInput did not send the move");

    let mut raw = Vec::new();
    for _ in 0..100 {
        el.poll_events(&mut EventSink::new(&mut q));
        raw.extend(q.drain().filter_map(|e| match e.event {
            WindowEvent::RawMouseMotion { dx, dy } => Some((dx, dy)),
            _ => None,
        }));
        if !raw.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(!raw.is_empty(), "no RawMouseMotion after SendInput");
    let (dx, dy) = raw.iter().fold((0, 0), |a, b| (a.0 + b.0, a.1 + b.1));
    assert!(dx > 0 && dy < 0, "expected right and up, got ({dx}, {dy})");
}
