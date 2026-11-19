// SPDX-License-Identifier: MIT OR Apache-2.0
//! `Window::set_ime_cursor_area` reaches the input method. The test reads the composition and
//! candidate forms back through the window's IME context. It skips when the window has no
//! context, which happens when no input method is active for the keyboard.
#![cfg(windows)]

use std::ffi::c_void;

use raw_window_handle::RawWindowHandle;
use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::Foundation::HWND;
use windows_sys::Win32::UI::Input::Ime::{
    CANDIDATEFORM, COMPOSITIONFORM, ImmGetCandidateWindow, ImmGetCompositionWindow, ImmGetContext,
    ImmReleaseContext,
};

fn pump(el: &mut EventLoop, q: &mut EventQueue, n: usize) {
    for _ in 0..n {
        el.poll_events(&mut EventSink::new(q));
        q.clear();
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn ime_area_is_applied_to_the_composition_and_candidate_forms() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(1024);
    let w = WindowBuilder::new()
        .with_title("tpt-window ime area test")
        .with_visible(false)
        .build(&el)
        .expect("window");
    pump(&mut el, &mut q, 5);

    w.set_ime_cursor_area(PhysicalPosition::new(120, 40), PhysicalSize::new(10, 20));
    pump(&mut el, &mut q, 5);

    let hwnd: HWND = match w.raw_window_handle().expect("handle") {
        RawWindowHandle::Win32(h) => (h.hwnd.get() as usize) as *mut c_void as HWND,
        other => panic!("expected a Win32 handle, got {other:?}"),
    };
    // SAFETY: `hwnd` is a live window owned by `w`, used on its thread. The context is
    // released before the end of the block.
    let (composition, candidate, ok) = unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.is_null() {
            (None, None, false)
        } else {
            let mut comp: COMPOSITIONFORM = std::mem::zeroed();
            let mut cand: CANDIDATEFORM = std::mem::zeroed();
            let got_c = ImmGetCompositionWindow(himc, &mut comp) != 0;
            let got_k = ImmGetCandidateWindow(himc, 0, &mut cand) != 0;
            ImmReleaseContext(hwnd, himc);
            (got_c.then_some(comp), got_k.then_some(cand), got_c && got_k)
        }
    };
    if !ok {
        eprintln!("skipping: the window has no IME context (no input method is active)");
        return;
    }
    let composition = composition.unwrap();
    let candidate = candidate.unwrap();
    assert_eq!(
        (composition.ptCurrentPos.x, composition.ptCurrentPos.y),
        (120, 40),
        "composition position"
    );
    // The candidate list opens just below the area: y + height.
    assert_eq!(
        (candidate.ptCurrentPos.x, candidate.ptCurrentPos.y),
        (120, 60),
        "candidate position"
    );
}
