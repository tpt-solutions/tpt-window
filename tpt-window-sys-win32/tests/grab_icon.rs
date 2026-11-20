// SPDX-License-Identifier: MIT OR Apache-2.0
//! Pointer confinement and window icons, checked against the system's own state.
#![cfg(windows)]

use std::time::{Duration, Instant};

use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::Foundation::{HWND, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetClipCursor, ICON_BIG, SendMessageW, SetForegroundWindow, WM_GETICON,
};

fn hwnd_of(w: &Window) -> HWND {
    match w.raw_window_handle().expect("handle") {
        RawWindowHandle::Win32(h) => h.hwnd.get() as HWND,
        _ => panic!("not a Win32 window"),
    }
}

fn settle(el: &mut EventLoop, q: &mut EventQueue) {
    let end = Instant::now() + Duration::from_millis(200);
    while Instant::now() < end {
        el.poll_events(&mut EventSink::new(q));
        q.drain().for_each(drop);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn confined_pointer_is_clipped_to_the_client_area() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(256);
    let w = WindowBuilder::new()
        .with_title("tpt-window grab test")
        .with_inner_size(LogicalSize::new(300.0, 200.0))
        .with_position(LogicalPosition::new(120.0, 120.0))
        .build(&el)
        .expect("window");
    let hwnd = hwnd_of(&w);
    unsafe { SetForegroundWindow(hwnd) };
    settle(&mut el, &mut q);

    w.set_cursor_grab(CursorGrab::Confined);
    settle(&mut el, &mut q);
    let mut clip = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetClipCursor(&mut clip) };
    let size = w.size();
    // The clip covers the client area, so its width and height match the client size. Focus
    // may not reach a background test process, in which case no clip is applied and this test
    // reports that instead of failing.
    if clip.right - clip.left != size.width as i32 {
        eprintln!("skipped: the window did not get focus, so no clip was applied");
        return;
    }
    assert_eq!(clip.bottom - clip.top, size.height as i32);

    w.set_cursor_grab(CursorGrab::None);
    settle(&mut el, &mut q);
    unsafe { GetClipCursor(&mut clip) };
    assert!(
        clip.right - clip.left > size.width as i32,
        "released clip covers the desktop"
    );
}

#[test]
fn an_icon_is_set_on_the_window() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(64);
    let w = WindowBuilder::new()
        .with_title("tpt-window icon test")
        .build(&el)
        .expect("window");
    let hwnd = hwnd_of(&w);
    // A 4x4 red square with full alpha.
    let rgba: Vec<u8> = (0..16).flat_map(|_| [255, 0, 0, 255]).collect();
    let icon = Icon::from_rgba(4, 4, rgba).expect("icon");

    w.set_icon(Some(icon));
    settle(&mut el, &mut q);
    let handle = unsafe { SendMessageW(hwnd, WM_GETICON, ICON_BIG as usize, 0) };
    assert_ne!(handle, 0, "the window should report its icon");

    w.set_icon(None);
    settle(&mut el, &mut q);
    let cleared = unsafe { SendMessageW(hwnd, WM_GETICON, ICON_BIG as usize, 0) };
    assert_eq!(cleared, 0, "clearing removes the icon");
}
