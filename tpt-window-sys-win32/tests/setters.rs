// SPDX-License-Identifier: MIT OR Apache-2.0
//! Runtime window setters change the real Win32 window, checked with the system's own
//! queries rather than our cached state.
#![cfg(windows)]

use std::time::Duration;

use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GWL_STYLE, GetWindowLongW, IsZoomed, WS_THICKFRAME,
};

fn hwnd_of(w: &Window) -> HWND {
    match w.raw_window_handle().expect("handle") {
        RawWindowHandle::Win32(h) => h.hwnd.get() as HWND,
        _ => panic!("not a Win32 window"),
    }
}

/// Pumps messages for a while so the command is applied and the system reports it.
fn pump(el: &mut EventLoop, q: &mut EventQueue, ms: u64) {
    let end = std::time::Instant::now() + Duration::from_millis(ms);
    while std::time::Instant::now() < end {
        el.poll_events(&mut EventSink::new(q));
        q.drain().for_each(drop);
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn maximize_and_restore_reach_the_system_window() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window setters test")
        .with_inner_size(LogicalSize::new(300.0, 200.0))
        .build(&el)
        .expect("window");
    let hwnd = hwnd_of(&w);

    w.set_maximized(true);
    pump(&mut el, &mut q, 300);
    assert!(unsafe { IsZoomed(hwnd) } != 0, "window should be maximized");

    w.set_maximized(false);
    pump(&mut el, &mut q, 300);
    assert!(unsafe { IsZoomed(hwnd) } == 0, "window should be restored");
}

#[test]
fn resizable_false_removes_the_sizing_frame() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window setters test 2")
        .with_inner_size(LogicalSize::new(300.0, 200.0))
        .build(&el)
        .expect("window");
    let hwnd = hwnd_of(&w);
    assert!(unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & WS_THICKFRAME != 0);

    w.set_resizable(false);
    pump(&mut el, &mut q, 100);
    assert!(unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & WS_THICKFRAME == 0);

    w.set_resizable(true);
    pump(&mut el, &mut q, 100);
    assert!(unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32 & WS_THICKFRAME != 0);
}
