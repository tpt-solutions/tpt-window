// SPDX-License-Identifier: MIT OR Apache-2.0
//! Decorations and fullscreen change the real window, checked with the system's own styles and
//! sizes.
#![cfg(windows)]

use std::time::{Duration, Instant};

use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{GWL_STYLE, GetWindowLongW, WS_CAPTION};

fn hwnd_of(w: &Window) -> HWND {
    match w.raw_window_handle().expect("handle") {
        RawWindowHandle::Win32(h) => h.hwnd.get() as HWND,
        _ => panic!("not a Win32 window"),
    }
}

/// Pumps messages for a short, fixed time so the system has applied the last change.
fn settle(el: &mut EventLoop, q: &mut EventQueue) {
    let end = Instant::now() + Duration::from_millis(150);
    while Instant::now() < end {
        el.poll_events(&mut EventSink::new(q));
        q.drain().for_each(drop);
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Pumps messages until `done` holds or a few seconds pass.
fn pump_until(el: &mut EventLoop, q: &mut EventQueue, mut done: impl FnMut() -> bool) {
    let end = Instant::now() + Duration::from_secs(3);
    while Instant::now() < end && !done() {
        el.poll_events(&mut EventSink::new(q));
        q.drain().for_each(drop);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn decorations_toggle_the_title_bar_and_keep_the_client_size() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window decorations test")
        .with_inner_size(LogicalSize::new(300.0, 200.0))
        .build(&el)
        .expect("window");
    let hwnd = hwnd_of(&w);
    settle(&mut el, &mut q);
    let client_before = w.size();
    let has_caption = |h: HWND| unsafe { GetWindowLongW(h, GWL_STYLE) } as u32 & WS_CAPTION != 0;
    assert!(has_caption(hwnd));

    w.set_decorations(false);
    pump_until(&mut el, &mut q, || !has_caption(hwnd));
    assert!(!has_caption(hwnd), "title bar should be gone");
    settle(&mut el, &mut q);
    assert_eq!(w.size(), client_before, "client size is kept");

    w.set_decorations(true);
    pump_until(&mut el, &mut q, || has_caption(hwnd));
    assert!(has_caption(hwnd), "title bar should be back");
}

#[test]
fn fullscreen_covers_the_display_and_restores_the_size() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window fullscreen test")
        .with_inner_size(LogicalSize::new(300.0, 200.0))
        .with_position(LogicalPosition::new(100.0, 100.0))
        .build(&el)
        .expect("window");
    settle(&mut el, &mut q);
    let windowed = w.size();
    let display = w.current_monitor().expect("a display").size;

    w.set_fullscreen(true);
    pump_until(&mut el, &mut q, || w.size() == display);
    assert_eq!(w.size(), display, "fullscreen covers the display");

    w.set_fullscreen(false);
    pump_until(&mut el, &mut q, || w.size() == windowed);
    assert_eq!(w.size(), windowed, "leaving fullscreen restores the size");
}
