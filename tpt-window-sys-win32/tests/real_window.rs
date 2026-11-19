// SPDX-License-Identifier: MIT OR Apache-2.0
//! Drives a real (hidden) Win32 window by posting messages to it.
#![cfg(windows)]

use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

use raw_window_handle::RawWindowHandle;
use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::UI::WindowsAndMessaging::*;

struct Counting;
thread_local! { static ALLOCS: Cell<usize> = const { Cell::new(0) }; }
fn bump() {
    let _ = ALLOCS.try_with(|c| c.set(c.get() + 1));
}
// SAFETY: forwards to the system allocator.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        bump();
        unsafe { System.alloc(l) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        bump();
        unsafe { System.realloc(p, l, n) }
    }
}
#[global_allocator]
static A: Counting = Counting;

fn hwnd_of(w: &Window) -> isize {
    match w.raw_window_handle().unwrap() {
        RawWindowHandle::Win32(h) => h.hwnd.get(),
        other => panic!("unexpected handle {other:?}"),
    }
}

fn pump(el: &mut EventLoop, q: &mut EventQueue) -> Vec<WindowEvent> {
    el.poll_events(&mut EventSink::new(q));
    q.drain().map(|e| e.event).collect()
}

#[test]
fn real_window_lifecycle_and_hot_path() {
    let mut el = new_event_loop().unwrap();
    let mut q = EventQueue::with_capacity(8192);
    let w = WindowBuilder::new()
        .with_title("tpt-window test")
        .with_inner_size(LogicalSize::new(400.0, 300.0))
        .with_visible(false)
        .build(&el)
        .unwrap();
    let hwnd = hwnd_of(&w) as *mut core::ffi::c_void;
    assert!(w.size().width > 0 && w.size().height > 0);
    assert!(w.scale_factor() >= 1.0);
    assert!(!w.is_visible());
    assert!(w.surface_handle().is_none());
    pump(&mut el, &mut q); // drain creation-time events

    // Setter is applied on the next poll, and produces a Resized event.
    w.set_size(PhysicalSize::new(500, 320));
    let evs = pump(&mut el, &mut q);
    assert!(
        evs.contains(&WindowEvent::Resized(PhysicalSize::new(500, 320))),
        "got {evs:?}"
    );
    assert_eq!(w.size(), PhysicalSize::new(500, 320));

    // Keyboard: 'A' key, scancode 0x1E (physical A on any layout).
    unsafe { PostMessageW(hwnd, WM_KEYDOWN, 0x41, 0x1E << 16) };
    let evs = pump(&mut el, &mut q);
    assert!(
        evs.contains(&WindowEvent::KeyboardInput {
            state: ElementState::Pressed,
            key_code: KeyCode::A,
            physical_key: PhysicalKey::Code(KeyCode::A),
        }),
        "got {evs:?}"
    );

    // Text: a non-ASCII character becomes an inline commit.
    unsafe { PostMessageW(hwnd, WM_CHAR, 0xE9, 0) };
    let evs = pump(&mut el, &mut q);
    assert!(
        evs.iter().any(|e| matches!(e,
            WindowEvent::Ime(ImeEvent::Commit { text, .. }) if text.as_str() == "é")),
        "got {evs:?}"
    );

    // Hot path: no allocations while pumping a burst of real messages.
    for i in 0..2000isize {
        unsafe { PostMessageW(hwnd, WM_MOUSEMOVE, 0, (i % 300) | (50 << 16)) };
    }
    let before = ALLOCS.with(Cell::get);
    let mut moves = 0;
    for _ in 0..4 {
        el.poll_events(&mut EventSink::new(&mut q));
        moves += q
            .drain()
            .filter(|e| matches!(e.event, WindowEvent::CursorMoved { .. }))
            .count();
    }
    let allocs = ALLOCS.with(Cell::get) - before;
    assert!(moves >= 2000, "only saw {moves} cursor moves");
    assert_eq!(allocs, 0, "Win32 poll hot path allocated");

    // Close request is reported, not acted on.
    unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) };
    assert!(pump(&mut el, &mut q).contains(&WindowEvent::CloseRequested));

    // Dropping the last handle destroys the window.
    drop(w);
    let evs = pump(&mut el, &mut q);
    assert!(evs.contains(&WindowEvent::Destroyed), "got {evs:?}");

    el.request_exit();
    assert_eq!(
        el.poll_events(&mut EventSink::new(&mut q)),
        PollStatus::Exit
    );
}
