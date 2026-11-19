// SPDX-License-Identifier: MIT OR Apache-2.0
//! Injects a real touch contact into a visible Win32 window and checks the event.
//!
//! Only the down (contact) is checked. On the development machine an injected lift
//! (`POINTER_FLAG_UP`) returns ERROR_TIMEOUT, so the `Ended` path is not verified
//! here yet (see todo.md).
#![cfg(windows)]

use raw_window_handle::RawWindowHandle;
use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::Foundation::{GetLastError, HWND, POINT};
use windows_sys::Win32::Graphics::Gdi::ClientToScreen;
use windows_sys::Win32::UI::Input::Pointer::{
    InitializeTouchInjection, InjectTouchInput, POINTER_FLAG_DOWN, POINTER_FLAG_INCONTACT,
    POINTER_FLAG_INRANGE, POINTER_TOUCH_INFO, TOUCH_FEEDBACK_NONE,
};
use windows_sys::Win32::UI::WindowsAndMessaging::PT_TOUCH;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
};

fn contact(x: i32, y: i32) -> POINTER_TOUCH_INFO {
    let mut info = POINTER_TOUCH_INFO::default();
    info.pointerInfo.pointerType = PT_TOUCH;
    info.pointerInfo.pointerId = 0;
    info.pointerInfo.ptPixelLocation = POINT { x, y };
    info.pointerInfo.pointerFlags =
        POINTER_FLAG_DOWN | POINTER_FLAG_INRANGE | POINTER_FLAG_INCONTACT;
    // A zero touch mask (no contact geometry) is the form that injects here. The
    // backend reads only the contact point.
    info.touchMask = 0;
    info
}

/// Polls for up to about a second, collecting events until `done` returns true.
fn collect(
    el: &mut EventLoop,
    q: &mut EventQueue,
    done: impl Fn(&[WindowEvent]) -> bool,
) -> Vec<WindowEvent> {
    let mut all = Vec::new();
    for _ in 0..100 {
        el.poll_events(&mut EventSink::new(q));
        all.extend(q.drain().map(|e| e.event));
        if done(&all) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    all
}

/// The contact moves across threads, so it needs a `Send` wrapper. Its only pointer
/// field (the HANDLE) is null.
struct SendContact(POINTER_TOUCH_INFO);
// SAFETY: see above; the contact holds no thread-bound resource.
unsafe impl Send for SendContact {}

impl SendContact {
    // A method call makes the closure capture the whole wrapper, not its field.
    fn into_inner(self) -> POINTER_TOUCH_INFO {
        self.0
    }
}

#[test]
fn injected_touch_down_is_reported_at_the_contact_point() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window touch test")
        .with_position(LogicalPosition::new(100.0, 100.0))
        .with_inner_size(LogicalSize::new(400.0, 300.0))
        .build(&el)
        .expect("window");
    // Let the window come up before touching it. Make it topmost so the contact at
    // its center lands on it and not on another window.
    collect(&mut el, &mut q, |_| w.is_visible());
    if let Ok(RawWindowHandle::Win32(h)) = w.raw_window_handle() {
        let hwnd = h.hwnd.get() as HWND;
        unsafe { SetWindowPos(hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE) };
    }
    collect(&mut el, &mut q, |_| false);
    let pos = w.position();
    let size = w.size();
    let (cx, cy) = (
        pos.x + size.width as i32 / 2,
        pos.y + size.height as i32 / 2,
    );

    let ok = unsafe { InitializeTouchInjection(10, TOUCH_FEEDBACK_NONE) };
    assert_ne!(ok, 0, "InitializeTouchInjection failed");

    // InjectTouchInput waits until the target thread handles the contact, so inject
    // from another thread while this one keeps pumping messages.
    let contact = SendContact(contact(cx, cy));
    let injector = std::thread::spawn(move || {
        let arr = [contact.into_inner()];
        let r = unsafe { InjectTouchInput(1, arr.as_ptr()) };
        (r, unsafe { GetLastError() })
    });
    let evs = collect(&mut el, &mut q, |evs| {
        evs.iter()
            .any(|e| matches!(e, WindowEvent::Touch(t) if t.phase == TouchPhase::Started))
    });
    let (r, err) = injector.join().expect("inject thread");
    assert_ne!(r, 0, "touch down injection failed: GetLastError={err}");

    let touches: Vec<TouchEvent> = evs
        .iter()
        .filter_map(|e| match e {
            WindowEvent::Touch(t) => Some(*t),
            _ => None,
        })
        .collect();
    let started = touches
        .iter()
        .find(|t| t.phase == TouchPhase::Started)
        .unwrap_or_else(|| panic!("no Started touch in {evs:?}"));
    // The contact was injected at the center of the outer frame. Client coordinates
    // are relative to the client origin, which sits inside the frame.
    let mut origin = POINT { x: 0, y: 0 };
    if let Ok(RawWindowHandle::Win32(h)) = w.raw_window_handle() {
        unsafe { ClientToScreen(h.hwnd.get() as HWND, &mut origin) };
    }
    let expected = (f64::from(cx - origin.x), f64::from(cy - origin.y));
    assert!(
        (started.position.x - expected.0).abs() <= 2.0
            && (started.position.y - expected.1).abs() <= 2.0,
        "touch at {:?}, expected {expected:?}",
        started.position
    );
}
