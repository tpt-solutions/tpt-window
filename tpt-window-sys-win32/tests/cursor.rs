// SPDX-License-Identifier: MIT OR Apache-2.0
//! Hiding the pointer changes what the system reports as the current cursor.
#![cfg(windows)]

use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursor, HTCLIENT, SendMessageW, WM_SETCURSOR,
};

fn hwnd_of(w: &Window) -> HWND {
    match w.raw_window_handle().expect("handle") {
        RawWindowHandle::Win32(h) => h.hwnd.get() as HWND,
        _ => panic!("not a Win32 window"),
    }
}

/// Applies queued commands, then asks the window for its cursor the way the system does.
fn cursor_after(el: &mut EventLoop, q: &mut EventQueue, hwnd: HWND) -> isize {
    el.poll_events(&mut EventSink::new(q));
    q.drain().for_each(drop);
    unsafe { SendMessageW(hwnd, WM_SETCURSOR, hwnd as WPARAM, HTCLIENT as LPARAM) };
    unsafe { GetCursor() as isize }
}

#[test]
fn hidden_pointer_is_reported_as_no_cursor_and_shown_again() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window cursor test")
        .build(&el)
        .expect("window");
    let hwnd = hwnd_of(&w);

    w.set_cursor_visible(false);
    assert_eq!(
        cursor_after(&mut el, &mut q, hwnd),
        0,
        "hidden: no cursor image"
    );

    w.set_cursor_visible(true);
    assert_ne!(cursor_after(&mut el, &mut q, hwnd), 0, "shown: arrow again");
}
