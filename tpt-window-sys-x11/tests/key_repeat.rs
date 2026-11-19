// SPDX-License-Identifier: MIT OR Apache-2.0
//! Held-key auto-repeat on a real X server, driven through XTest. Skips when `DISPLAY`
//! is unset or the server has no XTest, so it runs in CI only on the Xvfb job.
#![cfg(target_os = "linux")]

use std::ffi::c_int;
use std::time::{Duration, Instant};

use tpt_window_core::*;
use tpt_window_sys_x11::new_event_loop;
use x11_dl::{xlib, xtest};

/// Keysym for the lowercase letter `a` (X11 `XK_a`).
const XK_A: std::ffi::c_ulong = 0x61;

fn pump(el: &mut EventLoop, q: &mut EventQueue, out: &mut Vec<WindowEvent>) {
    el.poll_events(&mut EventSink::new(q));
    out.extend(q.drain().map(|e| e.event));
}

#[test]
fn held_key_sends_repeated_presses_then_one_release() {
    if std::env::var_os("DISPLAY").is_none() {
        eprintln!("skipping: no DISPLAY");
        return;
    }
    let Ok(xlib) = xlib::Xlib::open() else {
        eprintln!("skipping: libX11 not available");
        return;
    };
    let Ok(xtst) = xtest::Xf86vmode::open() else {
        eprintln!("skipping: libXtst not available");
        return;
    };
    // SAFETY: a null name opens the default display; it is closed at the end of the test.
    let dpy = unsafe { (xlib.XOpenDisplay)(std::ptr::null()) };
    if dpy.is_null() {
        eprintln!("skipping: cannot open display");
        return;
    }
    // SAFETY: `dpy` is a live display for the rest of the test.
    let (mut major, mut event, mut error) = (0, 0, 0);
    let has_xtest =
        unsafe { (xtst.XTestQueryExtension)(dpy, &mut event, &mut error, &mut major, &mut 0) != 0 };
    if !has_xtest {
        eprintln!("skipping: no XTEST extension");
        unsafe { (xlib.XCloseDisplay)(dpy) };
        return;
    }

    let mut el = new_event_loop().expect("open the X display");
    let mut q = EventQueue::with_capacity(4096);
    let w = WindowBuilder::new()
        .with_title("tpt-window key repeat test")
        .with_inner_size(LogicalSize::new(400.0, 300.0))
        .with_position(LogicalPosition::new(100.0, 100.0))
        .build(&el)
        .expect("create window");
    let mut seen = Vec::new();
    for _ in 0..20 {
        pump(&mut el, &mut q, &mut seen);
        std::thread::sleep(Duration::from_millis(10));
    }

    // Without a window manager X delivers keys to the window under the pointer.
    let pos = w.position();
    let size = w.size();
    let (x, y) = (
        pos.x + size.width as i32 / 2,
        pos.y + size.height as i32 / 2,
    );
    // SAFETY: plain Xlib/XTest calls on the live display.
    unsafe {
        (xtst.XTestFakeMotionEvent)(dpy, -1, x as c_int, y as c_int, 0);
        (xlib.XFlush)(dpy);
    }
    std::thread::sleep(Duration::from_millis(100));
    pump(&mut el, &mut q, &mut seen);
    seen.clear();

    // SAFETY: as above.
    let keycode = unsafe { (xlib.XKeysymToKeycode)(dpy, XK_A) } as u32;
    assert!(keycode != 0, "no keycode for 'a'");
    unsafe {
        (xtst.XTestFakeKeyEvent)(dpy, keycode, xlib::True, 0);
        (xlib.XFlush)(dpy);
    }
    // Hold the key long enough for the server's repeat delay (typically ~500 ms) and rate.
    let until = Instant::now() + Duration::from_millis(1500);
    while Instant::now() < until {
        pump(&mut el, &mut q, &mut seen);
        std::thread::sleep(Duration::from_millis(5));
    }
    unsafe {
        (xtst.XTestFakeKeyEvent)(dpy, keycode, xlib::False, 0);
        (xlib.XFlush)(dpy);
    }
    let until = Instant::now() + Duration::from_millis(300);
    while Instant::now() < until {
        pump(&mut el, &mut q, &mut seen);
        std::thread::sleep(Duration::from_millis(5));
    }

    let keys: Vec<ElementState> = seen
        .iter()
        .filter_map(|e| match e {
            WindowEvent::KeyboardInput {
                state,
                key_code: KeyCode::A,
                ..
            } => Some(*state),
            _ => None,
        })
        .collect();
    let presses = keys.iter().filter(|s| **s == ElementState::Pressed).count();
    let releases = keys
        .iter()
        .filter(|s| **s == ElementState::Released)
        .count();
    eprintln!("A events: {presses} presses, {releases} releases");

    assert!(presses > 1, "no auto-repeat: {keys:?}");
    assert_eq!(releases, 1, "repeats must not add releases: {keys:?}");
    assert_eq!(
        keys.last(),
        Some(&ElementState::Released),
        "the release must come last: {keys:?}"
    );

    drop(w);
    unsafe { (xlib.XCloseDisplay)(dpy) };
}
