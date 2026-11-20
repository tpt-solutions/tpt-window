// SPDX-License-Identifier: MIT OR Apache-2.0
//! Always-on-top sets the topmost bit the system reports for the window.
#![cfg(windows)]

use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_EXSTYLE, GetWindowLongW, WS_EX_TOPMOST};

#[test]
fn always_on_top_sets_and_clears_the_topmost_style() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(64);
    let w = WindowBuilder::new()
        .with_title("tpt-window always-on-top test")
        .build(&el)
        .expect("window");
    let hwnd = match w.raw_window_handle().expect("handle") {
        RawWindowHandle::Win32(h) => h.hwnd.get() as HWND,
        _ => panic!("not a Win32 window"),
    };
    let topmost = |h: HWND| unsafe { GetWindowLongW(h, GWL_EXSTYLE) } as u32 & WS_EX_TOPMOST != 0;
    let settle = |el: &mut EventLoop, q: &mut EventQueue| {
        el.poll_events(&mut EventSink::new(q));
        q.drain().for_each(drop);
    };

    settle(&mut el, &mut q);
    assert!(!topmost(hwnd));
    w.set_always_on_top(true);
    settle(&mut el, &mut q);
    assert!(topmost(hwnd), "window should be topmost");
    w.set_always_on_top(false);
    settle(&mut el, &mut q);
    assert!(!topmost(hwnd), "window should be in the normal order");
}
