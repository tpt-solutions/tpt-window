// SPDX-License-Identifier: MIT OR Apache-2.0
//! Shift pressed and released with SendInput reaches a real window as `ModifiersChanged`.
#![cfg(windows)]

use std::time::Duration;

use tpt_window_core::RawWindowHandle;
use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_SHIFT,
};
use windows_sys::Win32::UI::WindowsAndMessaging::SetForegroundWindow;

fn key(up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VK_SHIFT,
                wScan: 0,
                dwFlags: if up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

/// Polls until `count` modifier reports arrive or the time runs out.
fn collect_modifiers(el: &mut EventLoop, q: &mut EventQueue, count: usize) -> Vec<ModifiersState> {
    let mut out = Vec::new();
    for _ in 0..200 {
        el.poll_events(&mut EventSink::new(q));
        out.extend(q.drain().filter_map(|e| match e.event {
            WindowEvent::ModifiersChanged(m) => Some(m),
            _ => None,
        }));
        if out.len() >= count {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    out
}

#[test]
fn shift_press_and_release_reach_the_window() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window modifiers test")
        .with_inner_size(LogicalSize::new(300.0, 200.0))
        .build(&el)
        .expect("window");

    // Key messages go to the foreground window, so make this one foreground.
    let RawWindowHandle::Win32(h) = w.raw_window_handle().expect("handle") else {
        panic!("not a Win32 window");
    };
    let hwnd = h.hwnd.get() as HWND;
    unsafe { SetForegroundWindow(hwnd) };
    collect_modifiers(&mut el, &mut q, 0); // settle focus events

    let sent = unsafe { SendInput(1, &key(false), core::mem::size_of::<INPUT>() as i32) };
    assert_eq!(sent, 1, "SendInput did not send Shift down");
    let down = collect_modifiers(&mut el, &mut q, 1);
    if down.is_empty() {
        // Some sessions refuse foreground activation for a background process.
        eprintln!("skipped: window did not receive key input (no foreground rights)");
        return;
    }
    assert!(
        down.last().is_some_and(|m| m.shift()),
        "Shift down: {down:?}"
    );

    let sent = unsafe { SendInput(1, &key(true), core::mem::size_of::<INPUT>() as i32) };
    assert_eq!(sent, 1, "SendInput did not send Shift up");
    let up = collect_modifiers(&mut el, &mut q, 1);
    assert!(up.last().is_some_and(|m| !m.shift()), "Shift up: {up:?}");
}
