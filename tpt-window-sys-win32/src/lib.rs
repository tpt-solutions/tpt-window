// SPDX-License-Identifier: MIT OR Apache-2.0
//! Win32 backend for tpt-window (wraps `windows-sys`, the raw-FFI crate from the
//! windows-rs project; MIT OR Apache-2.0).
//!
//! Design notes:
//! * The window procedure runs inside `poll`/`step` (it is dispatched by our own message
//!   pump), translates messages into `Event`s and pushes them into a pre-allocated
//!   thread-local ring. `poll` then moves them into the caller's sink. No allocation
//!   happens on that path.
//! * `Window` setters may be called from any thread: they enqueue a command and post a
//!   message; the command is applied on the loop thread during the next poll.
//! * Per-monitor-v2 DPI awareness is enabled when the loop is created.
//!
//! Known gaps (see todo.md): touch/pen input, `transparent`, gamepads, IME composition.
#![cfg(windows)]
#![warn(missing_docs)]
// This crate is the FFI boundary; unsafe is expected and confined here.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::num::NonZeroIsize;
use std::ptr::{null, null_mut};
use std::sync::atomic::{
    AtomicBool, AtomicI32, AtomicIsize, AtomicU8, AtomicU16, AtomicU64, Ordering,
};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use tpt_window_core::*;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{ClientToScreen, CreateBitmap, DeleteObject};
use windows_sys::Win32::Graphics::Gdi::{InvalidateRect, ScreenToClient, ValidateRect};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::{GetCurrentThreadId, INFINITE};
use windows_sys::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor,
    GetDpiForSystem, GetDpiForWindow, MDT_EFFECTIVE_DPI, SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::Input::Ime::{
    CANDIDATEFORM, CFS_CANDIDATEPOS, CFS_POINT, COMPOSITIONFORM, GCS_COMPSTR,
    ImmGetCompositionStringW, ImmGetContext, ImmReleaseContext, ImmSetCandidateWindow,
    ImmSetCompositionWindow,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
    VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows_sys::Win32::UI::Input::Touch::{
    CloseTouchInputHandle, GetTouchInputInfo, RegisterTouchWindow, TOUCHEVENTF_DOWN,
    TOUCHEVENTF_UP, TOUCHINPUT,
};
use windows_sys::Win32::UI::Input::{
    GetRawInputData, RAWINPUT, RAWINPUTDEVICE, RAWINPUTHEADER, RID_INPUT, RIDEV_INPUTSINK,
    RIDEV_REMOVE, RIM_TYPEMOUSE, RegisterRawInputDevices,
};
use windows_sys::Win32::UI::Shell::{DragAcceptFiles, DragFinish, DragQueryFileW, HDROP};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Not in the enabled windows-sys feature set; value from winuser.h.
const WM_MOUSELEAVE: u32 = 0x02A3;
/// Touch contact update; the handle is in `lparam`.
const WM_TOUCH: u32 = 0x0240;

/// Current IME composition string, or empty when there is none.
/// Longest composition read, in UTF-16 units. Longer compositions are cut to this length.
const COMPOSITION_UNITS: usize = 256;

/// Text held in a fixed buffer, so reading a composition does not allocate.
struct StackText {
    buf: [u8; COMPOSITION_UNITS * 4],
    len: usize,
}

impl StackText {
    fn new() -> Self {
        Self {
            buf: [0; COMPOSITION_UNITS * 4],
            len: 0,
        }
    }

    /// Appends `c` if it fits; returns false (and appends nothing) when it does not.
    fn push(&mut self, c: char) -> bool {
        let mut utf8 = [0_u8; 4];
        let encoded = c.encode_utf8(&mut utf8);
        if self.len + encoded.len() > self.buf.len() {
            return false;
        }
        self.buf[self.len..self.len + encoded.len()].copy_from_slice(encoded.as_bytes());
        self.len += encoded.len();
        true
    }

    fn as_str(&self) -> &str {
        // Only whole UTF-8 sequences are ever appended by `push`, so this always succeeds.
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

/// Decodes UTF-16 composition units into `out`. Unpaired surrogates become U+FFFD.
fn decode_units(units: &[u16], out: &mut StackText) {
    for c in char::decode_utf16(units.iter().copied()) {
        if !out.push(c.unwrap_or('\u{FFFD}')) {
            break;
        }
    }
}

fn composition_text(hwnd: HWND) -> StackText {
    let mut text = StackText::new();
    // SAFETY: the context is released before returning.
    let himc = unsafe { ImmGetContext(hwnd) };
    if himc.is_null() {
        return text;
    }
    let bytes = unsafe { ImmGetCompositionStringW(himc, GCS_COMPSTR, core::ptr::null_mut(), 0) };
    if bytes > 0 {
        let mut units = [0_u16; COMPOSITION_UNITS];
        let units_len = (bytes as usize / 2).min(COMPOSITION_UNITS);
        // SAFETY: the buffer holds `units_len` units, and the byte count passed matches it.
        unsafe {
            ImmGetCompositionStringW(
                himc,
                GCS_COMPSTR,
                units.as_mut_ptr().cast(),
                (units_len * 2) as u32,
            )
        };
        decode_units(&units[..units_len], &mut text);
    }
    unsafe { ImmReleaseContext(hwnd, himc) };
    text
}

fn push_preedit(window: WindowId, text: &str) {
    let mut rest = text;
    loop {
        let (chunk, next) = ImeText::take_chunk(rest);
        let continued = !next.is_empty();
        push_event(
            window,
            WindowEvent::Ime(ImeEvent::Preedit {
                text: chunk,
                continued,
            }),
        );
        if !continued {
            break;
        }
        rest = next;
    }
}
const WM_TPT_COMMAND: u32 = WM_APP + 1;
const WM_TPT_DESTROY: u32 = WM_APP + 2;
const CLASS_NAME: [u16; 17] = utf16z(b"tpt_window_class");

const fn utf16z<const N: usize>(s: &[u8]) -> [u16; N] {
    let mut out = [0u16; N];
    let mut i = 0;
    while i < s.len() {
        out[i] = s[i] as u16;
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Thread-local pending-event ring (filled by the window procedure)
// ---------------------------------------------------------------------------

thread_local! {
    /// Reused buffer for `GetRawInputData`, so `WM_INPUT` does not allocate.
    static SCRATCH_RAW: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(64));
    /// Reused buffer for `GetTouchInputInfo`, so `WM_TOUCH` does not allocate.
    static SCRATCH_TOUCH: RefCell<Vec<TOUCHINPUT>> = RefCell::new(Vec::with_capacity(10));
    static PENDING: RefCell<EventQueue> = RefCell::new(new_pending_queue());
}

fn push_event(window: WindowId, event: WindowEvent) {
    PENDING.with(|p| {
        p.borrow_mut().push(Event { window, event });
    });
}

// ---------------------------------------------------------------------------
// Per-window data (referenced from GWLP_USERDATA)
// ---------------------------------------------------------------------------

#[derive(Default, Clone, Copy)]
struct Limits {
    min: Option<LogicalSize<f64>>,
    max: Option<LogicalSize<f64>>,
}

struct WindowData {
    id: WindowId,
    state: Arc<SharedWindowState>,
    commands: Mutex<VecDeque<WindowCommand>>,
    limits: Mutex<Limits>,
    cursor_inside: AtomicBool,
    /// Whether the pointer image is shown over the client area.
    cursor_visible: AtomicBool,
    buttons_down: AtomicU8,
    /// Keys and buttons currently held, so focus loss can release them. Without this a key
    /// held through Alt-Tab stays down for the application.
    held_keys: Mutex<Vec<(u32, KeyCode, PhysicalKey)>>,
    held_buttons: Mutex<Vec<MouseButton>>,
    minimized: AtomicBool,
    /// Style and outer rectangle to return to when leaving fullscreen.
    fullscreen_restore: Mutex<Option<(u32, RECT)>>,
    /// Last modifier state reported, so `ModifiersChanged` is sent only on a change.
    modifiers: AtomicU8,
    /// Last theme reported (see `theme_bits`), so `ThemeChanged` is sent only on a change.
    theme: AtomicU8,
    /// Pointer grab: 0 none, 1 confined, 2 locked (see `grab_bits`).
    grab: AtomicU8,
    /// The window icon's `HICON` as an integer, or 0. Destroyed when replaced or on destroy.
    icon: AtomicIsize,
    high_surrogate: AtomicU16,
    // Last reported client size, to avoid duplicate Resized events.
    last_w: AtomicI32,
    last_h: AtomicI32,
}

unsafe fn data_of<'a>(hwnd: HWND) -> Option<&'a WindowData> {
    let p = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const WindowData;
    // SAFETY: the pointer is an `Arc::into_raw` stored at WM_NCCREATE and cleared at
    // WM_NCDESTROY, so it is valid for the duration of any message in between.
    unsafe { p.as_ref() }
}

fn client_size(hwnd: HWND) -> PhysicalSize<u32> {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetClientRect(hwnd, &mut r) };
    PhysicalSize::new(
        (r.right - r.left).max(0) as u32,
        (r.bottom - r.top).max(0) as u32,
    )
}

fn outer_pos(hwnd: HWND) -> PhysicalPosition<i32> {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetWindowRect(hwnd, &mut r) };
    PhysicalPosition::new(r.left, r.top)
}

fn style_of(hwnd: HWND) -> (u32, u32) {
    unsafe {
        (
            GetWindowLongW(hwnd, GWL_STYLE) as u32,
            GetWindowLongW(hwnd, GWL_EXSTYLE) as u32,
        )
    }
}

/// Outer size needed for a client size at `dpi`.
fn outer_size(client: PhysicalSize<u32>, style: u32, ex: u32, dpi: u32) -> (i32, i32) {
    let mut r = RECT {
        left: 0,
        top: 0,
        right: client.width as i32,
        bottom: client.height as i32,
    };
    unsafe { AdjustWindowRectExForDpi(&mut r, style, 0, ex, dpi) };
    (r.right - r.left, r.bottom - r.top)
}

// ---------------------------------------------------------------------------
// Key translation
// ---------------------------------------------------------------------------

const LETTERS: [KeyCode; 26] = {
    use KeyCode::*;
    [
        A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    ]
};
const DIGITS: [KeyCode; 10] = {
    use KeyCode::*;
    [
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    ]
};
const FKEYS: [KeyCode; 12] = {
    use KeyCode::*;
    [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12]
};
const F13_F24: [KeyCode; 12] = {
    use KeyCode::*;
    [F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24]
};
const NUMPAD: [KeyCode; 10] = {
    use KeyCode::*;
    [
        Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    ]
};

fn translate_vk(vk: u32, scancode: u32, extended: bool) -> KeyCode {
    use KeyCode::*;
    match vk {
        0x41..=0x5A => LETTERS[(vk - 0x41) as usize],
        0x30..=0x39 => DIGITS[(vk - 0x30) as usize],
        0x70..=0x7B => FKEYS[(vk - 0x70) as usize],
        0x60..=0x69 => NUMPAD[(vk - 0x60) as usize],
        0x25 => ArrowLeft,
        0x26 => ArrowUp,
        0x27 => ArrowRight,
        0x28 => ArrowDown,
        0x1B => Escape,
        0x0D => {
            if extended {
                NumpadEnter
            } else {
                Enter
            }
        }
        0x20 => Space,
        0x09 => Tab,
        0x08 => Backspace,
        0x2E => Delete,
        0x2D => Insert,
        0x24 => Home,
        0x23 => End,
        0x21 => PageUp,
        0x22 => PageDown,
        0x10 => {
            if scancode == 0x36 {
                ShiftRight
            } else {
                ShiftLeft
            }
        }
        0xA0 => ShiftLeft,
        0xA1 => ShiftRight,
        0x11 => {
            if extended {
                ControlRight
            } else {
                ControlLeft
            }
        }
        0xA2 => ControlLeft,
        0xA3 => ControlRight,
        0x12 => {
            if extended {
                AltRight
            } else {
                AltLeft
            }
        }
        0xA4 => AltLeft,
        0xA5 => AltRight,
        0x5B => SuperLeft,
        0x5C => SuperRight,
        0x14 => CapsLock,
        0xBD => Minus,
        0xBB => Equal,
        0xDB => BracketLeft,
        0xDD => BracketRight,
        0xDC => Backslash,
        0xBA => Semicolon,
        0xDE => Quote,
        0xC0 => Backquote,
        0xBC => Comma,
        0xBE => Period,
        0xBF => Slash,
        0x6A => NumpadMultiply,
        0x6B => NumpadAdd,
        0x6D => NumpadSubtract,
        0x6E => NumpadDecimal,
        0x6F => NumpadDivide,
        0x7C..=0x87 => F13_F24[(vk - 0x7C) as usize],
        0x90 => NumLock,
        0x91 => ScrollLock,
        0x2C => PrintScreen,
        0x13 => Pause,
        0x5D => Menu,
        0xE2 => IntlBackslash,
        0xB3 => MediaPlayPause,
        0xB2 => MediaStop,
        0xB0 => MediaTrackNext,
        0xB1 => MediaTrackPrevious,
        0xAF => AudioVolumeUp,
        0xAE => AudioVolumeDown,
        0xAD => AudioVolumeMute,
        _ => Unidentified,
    }
}

/// Layout-independent key from a set-1 scancode (letters and digits; others are
/// reported as `Unidentified(scancode)`).
fn physical_from_scancode(scancode: u32, extended: bool, logical: KeyCode) -> PhysicalKey {
    let letter = |base: u32, row: &[usize]| row.get((scancode - base) as usize).copied();
    // QWERTY rows in set-1 scancode order, as indices into LETTERS (A=0).
    const ROW1: [usize; 10] = [16, 22, 4, 17, 19, 24, 20, 8, 14, 15]; // QWERTYUIOP
    const ROW2: [usize; 9] = [0, 18, 3, 5, 6, 7, 9, 10, 11]; // ASDFGHJKL
    const ROW3: [usize; 7] = [25, 23, 2, 21, 1, 13, 12]; // ZXCVBNM
    if !extended {
        let idx = match scancode {
            0x10..=0x19 => letter(0x10, &ROW1),
            0x1E..=0x26 => letter(0x1E, &ROW2),
            0x2C..=0x32 => letter(0x2C, &ROW3),
            _ => None,
        };
        if let Some(i) = idx {
            return PhysicalKey::Code(LETTERS[i]);
        }
        if (0x02..=0x0B).contains(&scancode) {
            return PhysicalKey::Code(DIGITS[((scancode - 1) % 10) as usize]);
        }
    }
    if logical != KeyCode::Unidentified && !LETTERS.contains(&logical) && !DIGITS.contains(&logical)
    {
        // Non-letter keys keep their position-based identity on all common layouts.
        return PhysicalKey::Code(logical);
    }
    PhysicalKey::Unidentified(scancode | if extended { 0xE000 } else { 0 })
}

// ---------------------------------------------------------------------------
// Window procedure
// ---------------------------------------------------------------------------

fn lo_i16(l: LPARAM) -> i32 {
    (l & 0xFFFF) as u16 as i16 as i32
}
fn hi_i16(l: LPARAM) -> i32 {
    ((l >> 16) & 0xFFFF) as u16 as i16 as i32
}

fn mouse_button(msg: u32, wparam: WPARAM) -> Option<(MouseButton, ElementState)> {
    use ElementState::*;
    Some(match msg {
        WM_LBUTTONDOWN => (MouseButton::Left, Pressed),
        WM_LBUTTONUP => (MouseButton::Left, Released),
        WM_RBUTTONDOWN => (MouseButton::Right, Pressed),
        WM_RBUTTONUP => (MouseButton::Right, Released),
        WM_MBUTTONDOWN => (MouseButton::Middle, Pressed),
        WM_MBUTTONUP => (MouseButton::Middle, Released),
        WM_XBUTTONDOWN | WM_XBUTTONUP => {
            let state = if msg == WM_XBUTTONDOWN {
                Pressed
            } else {
                Released
            };
            let which = ((wparam >> 16) & 0xFFFF) as u16;
            (
                if which == 1 {
                    MouseButton::Back
                } else {
                    MouseButton::Forward
                },
                state,
            )
        }
        _ => return None,
    })
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let cs = lparam as *const CREATESTRUCTW;
        let data = unsafe { (*cs).lpCreateParams } as *const WindowData;
        // The window now owns the reference made in create_window, and WM_NCDESTROY releases it.
        UNCLAIMED_REF.with(|c| c.set(0));
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as _) };
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let Some(d) = (unsafe { data_of(hwnd) }) else {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    let id = d.id;

    match msg {
        WM_NCDESTROY => {
            // Give up raw mouse input if this window had it, so the next window gets it.
            release_raw_mouse_if_owner(hwnd);
            unsafe { ClipCursor(null()) };
            let icon = d.icon.swap(0, Ordering::Relaxed);
            if icon != 0 {
                unsafe { DestroyIcon(icon as HICON) };
            }
            let p = unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) } as *const WindowData;
            // SAFETY: balances the Arc::into_raw in create_window.
            drop(unsafe { Arc::from_raw(p) });
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        WM_CLOSE => {
            push_event(id, WindowEvent::CloseRequested);
            return 0;
        }
        WM_DESTROY => {
            d.state.update(|s| s.visible = false);
            push_event(id, WindowEvent::Destroyed);
        }
        WM_TPT_DESTROY => {
            unsafe { DestroyWindow(hwnd) };
            return 0;
        }
        WM_TPT_COMMAND => {
            loop {
                let cmd = d
                    .commands
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .pop_front();
                let Some(cmd) = cmd else { break };
                apply_command(hwnd, d, cmd);
            }
            return 0;
        }
        WM_GETMINMAXINFO => {
            let limits = *d.limits.lock().unwrap_or_else(|e| e.into_inner());
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            let scale = f64::from(dpi) / 96.0;
            let (style, ex) = style_of(hwnd);
            let mmi = lparam as *mut MINMAXINFO;
            if let Some(m) = limits.min {
                let (w, h) = outer_size(m.to_physical(scale), style, ex, dpi);
                unsafe { (*mmi).ptMinTrackSize = POINT { x: w, y: h } };
            }
            if let Some(m) = limits.max {
                let (w, h) = outer_size(m.to_physical(scale), style, ex, dpi);
                unsafe { (*mmi).ptMaxTrackSize = POINT { x: w, y: h } };
            }
            return 0;
        }
        WM_SIZE => {
            let minimized = wparam == SIZE_MINIMIZED as usize;
            // WM_SIZE arrives on every resize; report only real minimize/restore changes.
            if d.minimized.swap(minimized, Ordering::Relaxed) != minimized {
                push_event(id, WindowEvent::Occluded(minimized));
            }
            if !minimized {
                let size = client_size(hwnd);
                if d.last_w.swap(size.width as i32, Ordering::Relaxed) != size.width as i32
                    || d.last_h.swap(size.height as i32, Ordering::Relaxed) != size.height as i32
                {
                    d.state.update(|s| s.size = size);
                    push_event(id, WindowEvent::Resized(size));
                }
            }
            return 0;
        }
        WM_MOVE => {
            // A minimized window reports a position of (-32000, -32000); that is not a move.
            if unsafe { IsIconic(hwnd) } != 0 {
                return 0;
            }
            let pos = outer_pos(hwnd);
            d.state.update(|s| s.position = pos);
            push_event(id, WindowEvent::Moved(pos));
            return 0;
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            let f = msg == WM_SETFOCUS;
            d.state.update(|s| s.focused = f);
            if !f {
                release_held_input(id, d);
                // A clip is lost with focus; it is applied again on the next focus gain.
                unsafe { ClipCursor(null()) };
            } else {
                apply_clip(hwnd, d.grab.load(Ordering::Relaxed));
            }
            push_event(id, WindowEvent::Focused(f));
            return 0;
        }
        // The system took mouse capture or cancelled the mode, so the matching button-up
        // messages will not arrive. Release what is held.
        WM_CAPTURECHANGED | WM_CANCELMODE => {
            release_held_input(id, d);
            return 0;
        }
        WM_DPICHANGED => {
            let dpi = (wparam & 0xFFFF) as u32;
            let r = unsafe { *(lparam as *const RECT) };
            unsafe {
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            let scale = f64::from(dpi) / 96.0;
            let size = client_size(hwnd);
            d.state.update(|s| {
                s.scale_factor = scale;
                s.size = size;
            });
            push_event(
                id,
                WindowEvent::ScaleFactorChanged {
                    scale_factor: scale,
                    new_inner_size: size,
                },
            );
            return 0;
        }
        WM_PAINT => {
            unsafe { ValidateRect(hwnd, null()) };
            push_event(id, WindowEvent::RedrawRequested);
            return 0;
        }
        WM_ERASEBKGND => return 1,
        WM_MOUSEMOVE => {
            if !d.cursor_inside.swap(true, Ordering::Relaxed) {
                let mut t = TRACKMOUSEEVENT {
                    cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                unsafe { TrackMouseEvent(&mut t) };
                push_event(id, WindowEvent::CursorEntered);
            }
            push_event(
                id,
                WindowEvent::CursorMoved {
                    position: PhysicalPosition::new(
                        f64::from(lo_i16(lparam)),
                        f64::from(hi_i16(lparam)),
                    ),
                    pressure: 0.0,
                },
            );
            return 0;
        }
        WM_INPUT => {
            let header = size_of::<RAWINPUTHEADER>() as u32;
            let mut size = 0_u32;
            unsafe {
                GetRawInputData(
                    lparam as _,
                    RID_INPUT,
                    core::ptr::null_mut(),
                    &mut size,
                    header,
                )
            };
            if size > 0 {
                // Reuse a per-thread buffer: raw input arrives at mouse rate, so allocating
                // here would break the zero-allocation hot path.
                let mut buf = SCRATCH_RAW.with(|s| core::mem::take(&mut *s.borrow_mut()));
                buf.clear();
                buf.resize(size as usize, 0);
                let got = unsafe {
                    GetRawInputData(
                        lparam as _,
                        RID_INPUT,
                        buf.as_mut_ptr().cast(),
                        &mut size,
                        header,
                    )
                };
                if got != u32::MAX && buf.len() >= size_of::<RAWINPUT>() {
                    // SAFETY: the buffer holds a RAWINPUT written by the system; it is
                    // read unaligned because the Vec<u8> is not aligned for it.
                    let raw = unsafe { core::ptr::read_unaligned(buf.as_ptr().cast::<RAWINPUT>()) };
                    if raw.header.dwType == RIM_TYPEMOUSE {
                        // SAFETY: dwType says the union holds a mouse report.
                        let mouse = unsafe { raw.data.mouse };
                        // Absolute devices (touch screens, tablets) report absolute
                        // positions, so only relative motion is forwarded.
                        if mouse.usFlags & 1 == 0 {
                            push_event(
                                id,
                                WindowEvent::RawMouseMotion {
                                    dx: mouse.lLastX,
                                    dy: mouse.lLastY,
                                },
                            );
                        }
                    }
                }
                SCRATCH_RAW.with(|s| *s.borrow_mut() = buf);
            }
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        WM_IME_COMPOSITION => {
            // Composition text is reported as Preedit. Committed text is left to
            // DefWindowProc, which sends it as WM_CHAR and reaches the Commit path.
            if lparam as u32 & GCS_COMPSTR != 0 {
                push_preedit(id, composition_text(hwnd).as_str());
            }
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        WM_IME_ENDCOMPOSITION => {
            push_preedit(id, "");
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        WM_TOUCH => {
            let count = wparam;
            // Per-thread scratch, as for raw input: no allocation per touch message.
            let mut inputs = SCRATCH_TOUCH.with(|s| core::mem::take(&mut *s.borrow_mut()));
            inputs.clear();
            inputs.resize(count, unsafe { core::mem::zeroed() });
            let ok = unsafe {
                GetTouchInputInfo(
                    lparam as _,
                    count as u32,
                    inputs.as_mut_ptr(),
                    size_of::<TOUCHINPUT>() as i32,
                )
            };
            if ok != 0 {
                for input in &inputs {
                    // Touch coordinates are screen points in hundredths of a pixel.
                    let mut pt = POINT {
                        x: input.x / 100,
                        y: input.y / 100,
                    };
                    unsafe { ScreenToClient(hwnd, &mut pt) };
                    let phase = if input.dwFlags & TOUCHEVENTF_DOWN != 0 {
                        TouchPhase::Started
                    } else if input.dwFlags & TOUCHEVENTF_UP != 0 {
                        TouchPhase::Ended
                    } else {
                        TouchPhase::Moved
                    };
                    push_event(
                        id,
                        WindowEvent::Touch(TouchEvent {
                            id: u64::from(input.dwID),
                            phase,
                            position: PhysicalPosition::new(f64::from(pt.x), f64::from(pt.y)),
                            force: 0.0,
                        }),
                    );
                }
            }
            unsafe { CloseTouchInputHandle(lparam as _) };
            SCRATCH_TOUCH.with(|s| *s.borrow_mut() = inputs);
            return 0;
        }
        WM_DROPFILES => {
            let drop = wparam as HDROP;
            // SAFETY: `drop` is the HDROP the system sent for this message; it is released below.
            let count = unsafe { DragQueryFileW(drop, u32::MAX, null_mut(), 0) };
            for index in 0..count {
                let len = unsafe { DragQueryFileW(drop, index, null_mut(), 0) } as usize;
                let mut units = vec![0_u16; len + 1];
                let written =
                    unsafe { DragQueryFileW(drop, index, units.as_mut_ptr(), units.len() as u32) }
                        as usize;
                let path = String::from_utf16_lossy(&units[..written.min(len)]);
                push_path_events(id, &path, |chunk, continued| WindowEvent::FileDropped {
                    chunk,
                    continued,
                });
            }
            unsafe { DragFinish(drop) };
            return 0;
        }
        WM_SETTINGCHANGE => {
            // The theme is a registry value, so re-read it on any settings change, and report
            // only a real change.
            let theme = theme_bits(system_theme());
            if d.theme.swap(theme, Ordering::Relaxed) != theme {
                if let Some(t) = bits_theme(theme) {
                    push_event(id, WindowEvent::ThemeChanged(t));
                }
            }
        }
        WM_SETCURSOR => {
            // Windows sets the cursor on every mouse move, so hiding it here keeps it hidden.
            // Only the client area is affected; the frame keeps its normal cursors.
            if (lparam as u32 & 0xFFFF) == HTCLIENT && pointer_hidden(d) {
                unsafe { SetCursor(null_mut()) };
                return 1;
            }
        }
        WM_MOUSELEAVE => {
            d.cursor_inside.store(false, Ordering::Relaxed);
            push_event(id, WindowEvent::CursorLeft);
            return 0;
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN
        | WM_MBUTTONUP | WM_XBUTTONDOWN | WM_XBUTTONUP => {
            if let Some((button, state)) = mouse_button(msg, wparam) {
                match state {
                    ElementState::Pressed => {
                        track_held(&d.held_buttons, button, true);
                        if d.buttons_down.fetch_add(1, Ordering::Relaxed) == 0 {
                            unsafe { SetCapture(hwnd) };
                        }
                    }
                    ElementState::Released => {
                        track_held(&d.held_buttons, button, false);
                        let prev = d.buttons_down.load(Ordering::Relaxed);
                        d.buttons_down
                            .store(prev.saturating_sub(1), Ordering::Relaxed);
                        if prev <= 1 {
                            unsafe { ReleaseCapture() };
                        }
                    }
                }
                push_event(id, WindowEvent::MouseInput { state, button });
            }
            // XBUTTON messages must return TRUE.
            return if matches!(msg, WM_XBUTTONDOWN | WM_XBUTTONUP) {
                1
            } else {
                0
            };
        }
        WM_MOUSEWHEEL | WM_MOUSEHWHEEL => {
            let delta = ((wparam >> 16) & 0xFFFF) as u16 as i16 as f32 / 120.0;
            let (x, y) = if msg == WM_MOUSEWHEEL {
                (0.0, delta)
            } else {
                (delta, 0.0)
            };
            push_event(
                id,
                WindowEvent::MouseWheel {
                    delta: MouseScrollDelta::LineDelta { x, y },
                    phase: TouchPhase::Moved,
                },
            );
            return 0;
        }
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP => {
            let state = if matches!(msg, WM_KEYDOWN | WM_SYSKEYDOWN) {
                ElementState::Pressed
            } else {
                ElementState::Released
            };
            let scancode = ((lparam >> 16) & 0xFF) as u32;
            let extended = (lparam >> 24) & 1 == 1;
            let key_code = translate_vk(wparam as u32, scancode, extended);
            let physical_key = physical_from_scancode(scancode, extended, key_code);
            track_held_key(d, wparam as u32, key_code, physical_key, state);
            sync_modifiers(d, id);
            push_event(
                id,
                WindowEvent::KeyboardInput {
                    state,
                    key_code,
                    physical_key,
                },
            );
            // Let DefWindowProc handle Alt+F4 / Alt+Space etc.
            if msg == WM_KEYDOWN || msg == WM_KEYUP {
                return 0;
            }
        }
        WM_CHAR => {
            let unit = wparam as u16;
            // Any unit other than a high surrogate ends a pending pair, so a stale high
            // surrogate can never join with a later low surrogate.
            let pending_high = d.high_surrogate.swap(0, Ordering::Relaxed);
            if (0xD800..0xDC00).contains(&unit) {
                d.high_surrogate.store(unit, Ordering::Relaxed);
                return 0;
            }
            // Skip control characters (Backspace, Enter, Tab, Escape arrive as keys).
            if unit >= 0x20 && unit != 0x7F {
                if let Some(ch) = wm_char_to_char(pending_high, unit) {
                    let mut buf = [0u8; 4];
                    push_event(
                        id,
                        WindowEvent::Ime(ImeEvent::Commit {
                            text: ImeText::new(ch.encode_utf8(&mut buf)),
                            continued: false,
                        }),
                    );
                }
            }
            return 0;
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Adds or removes a button from the held list. Presses are de-duplicated, and a release for
/// a button that is not held is ignored.
fn track_held(held: &Mutex<Vec<MouseButton>>, button: MouseButton, down: bool) {
    let mut held = held.lock().unwrap_or_else(|e| e.into_inner());
    if down {
        if !held.contains(&button) {
            held.push(button);
        }
    } else {
        held.retain(|b| *b != button);
    }
}

/// Records a key press or release by virtual key. Auto-repeat presses are de-duplicated.
fn track_held_key(
    d: &WindowData,
    vk: u32,
    key_code: KeyCode,
    physical_key: PhysicalKey,
    state: ElementState,
) {
    let mut held = d.held_keys.lock().unwrap_or_else(|e| e.into_inner());
    match state {
        ElementState::Pressed => {
            if !held.iter().any(|(v, _, _)| *v == vk) {
                held.push((vk, key_code, physical_key));
            }
        }
        ElementState::Released => held.retain(|(v, _, _)| *v != vk),
    }
}

/// Emits a release for every key and button still held, and drops mouse capture. Called on
/// focus loss and when the system takes capture away, because the real release messages will
/// not arrive. Each release is reported once.
/// The modifier keys held right now. `GetKeyState` reports the state as this thread's message
/// queue has seen it, which is the state the key messages describe.
fn current_modifiers() -> ModifiersState {
    // The high bit is set while a key is down.
    let down = |vk: u16| unsafe { GetKeyState(i32::from(vk)) } < 0;
    ModifiersState::new(
        down(VK_SHIFT),
        down(VK_CONTROL),
        down(VK_MENU),
        down(VK_LWIN) || down(VK_RWIN),
    )
}

/// Sends `ModifiersChanged` if the held modifiers differ from the last report.
fn sync_modifiers(d: &WindowData, id: WindowId) {
    let mods = current_modifiers();
    if d.modifiers.swap(mods.bits(), Ordering::Relaxed) != mods.bits() {
        push_event(id, WindowEvent::ModifiersChanged(mods));
    }
}

fn release_held_input(id: WindowId, d: &WindowData) {
    loop {
        let next = d.held_keys.lock().unwrap_or_else(|e| e.into_inner()).pop();
        let Some((_, key_code, physical_key)) = next else {
            break;
        };
        push_event(
            id,
            WindowEvent::KeyboardInput {
                state: ElementState::Released,
                key_code,
                physical_key,
            },
        );
    }
    loop {
        let next = d
            .held_buttons
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .pop();
        let Some(button) = next else {
            break;
        };
        push_event(
            id,
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button,
            },
        );
    }
    if d.buttons_down.swap(0, Ordering::Relaxed) != 0 {
        unsafe { ReleaseCapture() };
    }
    // Focus loss clears the modifiers too, so a Shift held through Alt-Tab is not still held.
    if d.modifiers.swap(0, Ordering::Relaxed) != 0 {
        push_event(id, WindowEvent::ModifiersChanged(ModifiersState::empty()));
    }
}

/// Turns one `WM_CHAR` unit into a character, given the high surrogate that came before it
/// (0 when there was none). Returns `None` for a unit that is not a complete character:
/// a lone low surrogate, or a pair whose high half is missing. A lone low surrogate must not
/// decode as NUL.
fn wm_char_to_char(pending_high: u16, unit: u16) -> Option<char> {
    if (0xDC00..0xE000).contains(&unit) {
        if pending_high == 0 {
            return None;
        }
        return char::decode_utf16([pending_high, unit])
            .next()
            .and_then(Result::ok);
    }
    char::from_u32(u32::from(unit))
}

fn apply_command(hwnd: HWND, d: &WindowData, cmd: WindowCommand) {
    match cmd {
        WindowCommand::SetTitle(t) => {
            let wide: Vec<u16> = t.encode_utf16().chain(Some(0)).collect();
            unsafe { SetWindowTextW(hwnd, wide.as_ptr()) };
        }
        WindowCommand::SetSize(size) => {
            let (style, ex) = style_of(hwnd);
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            let (w, h) = outer_size(size, style, ex, dpi);
            unsafe {
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    w,
                    h,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
        }
        WindowCommand::SetPosition(p) => {
            unsafe {
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    p.x,
                    p.y,
                    0,
                    0,
                    SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
        }
        WindowCommand::SetVisible(v) => {
            unsafe { ShowWindow(hwnd, if v { SW_SHOW } else { SW_HIDE }) };
            d.state.update(|s| s.visible = v);
        }
        WindowCommand::SetImeCursorArea { position, size } => {
            set_ime_area(hwnd, position, size);
        }
        WindowCommand::RequestRedraw => {
            unsafe { InvalidateRect(hwnd, null(), 0) };
        }
        WindowCommand::SetMinSize(size) => {
            let scale = unsafe { GetDpiForWindow(hwnd) } as f64 / 96.0;
            let mut limits = d.limits.lock().unwrap_or_else(|e| e.into_inner());
            limits.min = size.map(|s| to_logical(s, scale));
        }
        WindowCommand::SetMaxSize(size) => {
            let scale = unsafe { GetDpiForWindow(hwnd) } as f64 / 96.0;
            let mut limits = d.limits.lock().unwrap_or_else(|e| e.into_inner());
            limits.max = size.map(|s| to_logical(s, scale));
        }
        WindowCommand::SetResizable(resizable) => {
            // Resizing is the sizing frame plus the maximize box. Changing them needs
            // SWP_FRAMECHANGED for the frame to be redrawn.
            let (style, _) = style_of(hwnd);
            let sizing = WS_THICKFRAME | WS_MAXIMIZEBOX;
            let style = if resizable {
                style | sizing
            } else {
                style & !sizing
            };
            unsafe {
                SetWindowLongW(hwnd, GWL_STYLE, style as i32);
                SetWindowPos(
                    hwnd,
                    null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
                );
            }
        }
        WindowCommand::SetMaximized(on) => {
            unsafe { ShowWindow(hwnd, if on { SW_MAXIMIZE } else { SW_RESTORE }) };
        }
        WindowCommand::SetAlwaysOnTop(on) => {
            let order = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
            unsafe {
                SetWindowPos(
                    hwnd,
                    order,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
        }
        WindowCommand::SetMinimized(on) => {
            unsafe { ShowWindow(hwnd, if on { SW_MINIMIZE } else { SW_RESTORE }) };
        }
        WindowCommand::SetDecorations(on) => {
            // Keep the client area: the frame changes size, so the outer size is recomputed for
            // the same client size under the new style.
            let fullscreen = d
                .fullscreen_restore
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_some();
            if !fullscreen {
                let client = client_size(hwnd);
                let (style, ex) = style_of(hwnd);
                let frame =
                    WS_CAPTION | WS_THICKFRAME | WS_SYSMENU | WS_MINIMIZEBOX | WS_MAXIMIZEBOX;
                let style = if on {
                    (style & !WS_POPUP) | frame
                } else {
                    (style & !frame) | WS_POPUP
                };
                let dpi = unsafe { GetDpiForWindow(hwnd) };
                let (w, h) = outer_size(client, style, ex, dpi);
                unsafe {
                    SetWindowLongW(hwnd, GWL_STYLE, style as i32);
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        0,
                        0,
                        w,
                        h,
                        SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    );
                }
            }
        }
        WindowCommand::SetFullscreen(on) => {
            let mut saved = d
                .fullscreen_restore
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            match (on, saved.take()) {
                (true, None) => {
                    let (style, _) = style_of(hwnd);
                    let mut rect: RECT = unsafe { core::mem::zeroed() };
                    unsafe { GetWindowRect(hwnd, &mut rect) };
                    *saved = Some((style, rect));
                    if let Some(bounds) = monitor_bounds(hwnd) {
                        unsafe {
                            SetWindowLongW(
                                hwnd,
                                GWL_STYLE,
                                (style & !(WS_CAPTION | WS_THICKFRAME) | WS_POPUP) as i32,
                            );
                            SetWindowPos(
                                hwnd,
                                null_mut(),
                                bounds.left,
                                bounds.top,
                                bounds.right - bounds.left,
                                bounds.bottom - bounds.top,
                                SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                            );
                        }
                    }
                }
                (false, Some((style, rect))) => unsafe {
                    SetWindowLongW(hwnd, GWL_STYLE, style as i32);
                    SetWindowPos(
                        hwnd,
                        null_mut(),
                        rect.left,
                        rect.top,
                        rect.right - rect.left,
                        rect.bottom - rect.top,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
                    );
                },
                (_, previous) => *saved = previous,
            }
        }
        WindowCommand::SetCursorVisible(visible) => {
            d.cursor_visible.store(visible, Ordering::Relaxed);
            apply_pointer_image(d);
        }
        WindowCommand::SetCursorGrab(grab) => {
            let bits = grab_bits(grab);
            d.grab.store(bits, Ordering::Relaxed);
            if d.state.snapshot().focused {
                apply_clip(hwnd, bits);
            }
            apply_pointer_image(d);
        }
        WindowCommand::SetIcon(icon) => {
            let new = icon.as_ref().map_or(0, make_hicon);
            let old = d.icon.swap(new, Ordering::Relaxed);
            unsafe {
                SendMessageW(hwnd, WM_SETICON, ICON_BIG as usize, new);
                SendMessageW(hwnd, WM_SETICON, ICON_SMALL as usize, new);
            }
            if old != 0 {
                unsafe { DestroyIcon(old as HICON) };
            }
        }
        _ => debug_assert!(
            false,
            "unhandled WindowCommand: give it an arm in this backend"
        ),
    }
}

/// Converts a physical size to logical units at `scale` (1.0 = 96 dpi).
fn to_logical(size: PhysicalSize<u32>, scale: f64) -> LogicalSize<f64> {
    LogicalSize::new(
        f64::from(size.width) / scale,
        f64::from(size.height) / scale,
    )
}

/// Places the input method's composition and candidate windows at `position`, which is
/// the top-left of the text being composed, with `size` as its extent. Returns `false` if
/// the window has no input context (no IME is active for it), in which case there is
/// nothing to place.
fn set_ime_area(hwnd: HWND, position: PhysicalPosition<i32>, size: PhysicalSize<u32>) -> bool {
    // SAFETY: `hwnd` is a live window of this thread. The forms are plain structs that
    // live for the duration of each call, and the context is released before returning.
    unsafe {
        let himc = ImmGetContext(hwnd);
        if himc.is_null() {
            return false;
        }
        let (x, y) = (position.x, position.y);
        let area = RECT {
            left: x,
            top: y,
            right: x + size.width as i32,
            bottom: y + size.height as i32,
        };
        let composition = COMPOSITIONFORM {
            dwStyle: CFS_POINT,
            ptCurrentPos: POINT { x, y },
            rcArea: area,
        };
        let candidate = CANDIDATEFORM {
            dwIndex: 0,
            dwStyle: CFS_CANDIDATEPOS,
            ptCurrentPos: POINT {
                x,
                y: y + size.height as i32,
            },
            rcArea: area,
        };
        ImmSetCompositionWindow(himc, &composition);
        ImmSetCandidateWindow(himc, &candidate);
        ImmReleaseContext(hwnd, himc);
    }
    true
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

/// Whether the window class is registered. A failed registration is retried by the next
/// window, so an error is not cached for the rest of the process.
static REGISTER: Mutex<bool> = Mutex::new(false);

thread_local! {
    /// The `Arc` reference handed to `CreateWindowExW` that no window has taken yet. Zero once
    /// `WM_NCCREATE` runs, because the window then owns it. Read after a failed create, to tell
    /// whether this side still has to give it back.
    static UNCLAIMED_REF: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
    /// The window that currently receives raw mouse input (0 for none).
    static RAW_MOUSE_OWNER: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// Clears raw mouse registration when `hwnd` is the window that holds it. Registering again
/// from another window replaces the target, so only the owner may remove it.
fn release_raw_mouse_if_owner(hwnd: HWND) {
    if RAW_MOUSE_OWNER.with(|c| c.get()) != hwnd as usize {
        return;
    }
    let mouse = RAWINPUTDEVICE {
        usUsagePage: 0x01,
        usUsage: 0x02,
        dwFlags: RIDEV_REMOVE,
        hwndTarget: null_mut(),
    };
    unsafe { RegisterRawInputDevices(&mouse, 1, size_of::<RAWINPUTDEVICE>() as u32) };
    RAW_MOUSE_OWNER.with(|c| c.set(0));
}
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn hinstance() -> *mut core::ffi::c_void {
    unsafe { GetModuleHandleW(null()) }
}

fn register_class() -> Result<(), WindowError> {
    let mut registered = REGISTER.lock().unwrap_or_else(|e| e.into_inner());
    if *registered {
        return Ok(());
    }
    let mut result = Ok(());
    {
        let wc = WNDCLASSEXW {
            cbSize: size_of::<WNDCLASSEXW>() as u32,
            style: CS_OWNDC,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinstance(),
            hIcon: null_mut(),
            hCursor: unsafe { LoadCursorW(null_mut(), IDC_ARROW) },
            hbrBackground: null_mut(),
            lpszMenuName: null(),
            lpszClassName: CLASS_NAME.as_ptr(),
            hIconSm: null_mut(),
        };
        if unsafe { RegisterClassExW(&wc) } == 0 {
            result = Err(WindowError::Os("RegisterClassExW failed".into()));
        }
    }
    *registered = result.is_ok();
    result
}

/// Win32 event loop backend. Create with [`new_event_loop`].
pub struct Win32EventLoop {
    exit: Arc<AtomicBool>,
    /// The thread that owns the loop. Thread messages posted to it wake a blocked `step`.
    thread_id: u32,
}

/// Sends a path as chunks, so a path of any length fits the fixed-size event payload.
fn push_path_events(window: WindowId, path: &str, make: impl Fn(ImeText, bool) -> WindowEvent) {
    let mut rest = path;
    loop {
        let (chunk, next) = ImeText::take_chunk(rest);
        let continued = !next.is_empty();
        push_event(window, make(chunk, continued));
        if !continued {
            break;
        }
        rest = next;
    }
}

/// Describes one display: name, bounds, effective DPI and whether it is primary.
fn monitor_info(
    hmon: windows_sys::Win32::Graphics::Gdi::HMONITOR,
) -> Option<tpt_window_core::MonitorInfo> {
    use windows_sys::Win32::Graphics::Gdi::{GetMonitorInfoW, MONITORINFO, MONITORINFOEXW};

    let mut info: MONITORINFOEXW = unsafe { core::mem::zeroed() };
    info.monitorInfo.cbSize = size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: the struct is sized as MONITORINFOEXW, which begins with a MONITORINFO.
    if unsafe {
        GetMonitorInfoW(
            hmon,
            (&mut info as *mut MONITORINFOEXW).cast::<MONITORINFO>(),
        )
    } == 0
    {
        return None;
    }
    let (mut dpi_x, mut dpi_y) = (0_u32, 0_u32);
    // SAFETY: both out-pointers are valid locals.
    unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    let r = info.monitorInfo.rcMonitor;
    let len = info
        .szDevice
        .iter()
        .position(|&c| c == 0)
        .unwrap_or(info.szDevice.len());
    let name = String::from_utf16_lossy(&info.szDevice[..len]);
    Some(tpt_window_core::MonitorInfo {
        name: (!name.is_empty()).then_some(name),
        position: PhysicalPosition::new(r.left, r.top),
        size: PhysicalSize::new(
            (r.right - r.left).max(0) as u32,
            (r.bottom - r.top).max(0) as u32,
        ),
        scale_factor: f64::from(dpi_x.max(96)) / 96.0,
        primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

/// The display a window is mostly on, or the nearest one if it is off every display.
fn window_monitor(hwnd: HWND) -> Option<tpt_window_core::MonitorInfo> {
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromWindow};
    // SAFETY: a plain query on a window handle this crate owns.
    let hmon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if hmon.is_null() {
        None
    } else {
        monitor_info(hmon)
    }
}

/// Encodes a grab for the atomic: 0 none, 1 confined, 2 locked.
fn grab_bits(grab: CursorGrab) -> u8 {
    match grab {
        CursorGrab::None => 0,
        CursorGrab::Confined => 1,
        CursorGrab::Locked => 2,
    }
}

/// Whether the pointer image is hidden over the client area: hidden by the application, or
/// locked by a grab.
fn pointer_hidden(d: &WindowData) -> bool {
    !d.cursor_visible.load(Ordering::Relaxed) || d.grab.load(Ordering::Relaxed) == 2
}

/// Shows the arrow or hides the pointer image now, if the pointer is over the window. Otherwise
/// the next `WM_SETCURSOR` applies it.
fn apply_pointer_image(d: &WindowData) {
    if d.cursor_inside.load(Ordering::Relaxed) {
        let image = if pointer_hidden(d) {
            null_mut()
        } else {
            unsafe { LoadCursorW(null_mut(), IDC_ARROW) }
        };
        unsafe { SetCursor(image) };
    }
}

/// Confines the pointer to the client area (`bits` 1 or 2), or releases it (0).
fn apply_clip(hwnd: HWND, bits: u8) {
    if bits == 0 {
        unsafe { ClipCursor(null()) };
        return;
    }
    let mut client = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    unsafe { GetClientRect(hwnd, &mut client) };
    let mut top_left = POINT {
        x: client.left,
        y: client.top,
    };
    let mut corner = POINT {
        x: client.right,
        y: client.bottom,
    };
    unsafe {
        ClientToScreen(hwnd, &mut top_left);
        ClientToScreen(hwnd, &mut corner);
    }
    let rect = RECT {
        left: top_left.x,
        top: top_left.y,
        right: corner.x,
        bottom: corner.y,
    };
    unsafe { ClipCursor(&rect) };
}

/// Builds an icon from straight RGBA pixels. The caller owns the returned handle, as an integer
/// (0 on failure), and destroys it.
fn make_hicon(icon: &tpt_window_core::Icon) -> isize {
    let (w, h) = (icon.width() as i32, icon.height() as i32);
    // GDI wants blue, green, red, alpha.
    let mut bgra = Vec::with_capacity(icon.rgba().len());
    for px in icon.rgba().chunks_exact(4) {
        bgra.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
    }
    // SAFETY: the bit buffer is w*h*4 bytes; the mask has no set bits, so the alpha channel
    // decides transparency. Both bitmaps are deleted after the icon is made.
    unsafe {
        let color = CreateBitmap(w, h, 1, 32, bgra.as_ptr().cast());
        let mask = CreateBitmap(w, h, 1, 1, null());
        let info = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask,
            hbmColor: color,
        };
        let hicon = CreateIconIndirect(&info);
        DeleteObject(color);
        DeleteObject(mask);
        hicon as isize
    }
}

/// The effective DPI of the display under a physical point, if there is one.
fn dpi_at_point(p: PhysicalPosition<i32>) -> Option<u32> {
    use windows_sys::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONEAREST, MonitorFromPoint};
    let pt = POINT { x: p.x, y: p.y };
    // SAFETY: a plain query with a value point.
    let hmon = unsafe { MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST) };
    if hmon.is_null() {
        return None;
    }
    let (mut dpi_x, mut dpi_y) = (0_u32, 0_u32);
    // SAFETY: both out-pointers are valid locals.
    unsafe { GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    (dpi_x > 0).then_some(dpi_x)
}

/// The current light or dark appearance, from the per-user registry value that Windows uses for
/// apps. `None` when the value cannot be read.
fn system_theme() -> Option<tpt_window_core::Theme> {
    use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
    // Stack buffers, not Vecs: this runs inside the window procedure, and a settings broadcast
    // can arrive during the zero-allocation poll path.
    fn wide<const N: usize>(text: &str) -> [u16; N] {
        let mut out = [0_u16; N];
        for (slot, unit) in out.iter_mut().zip(text.encode_utf16()) {
            *slot = unit;
        }
        out
    }
    let key = wide::<96>(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let name = wide::<32>("AppsUseLightTheme");
    let mut value: u32 = 0;
    let mut size = size_of::<u32>() as u32;
    // SAFETY: the names are NUL-terminated and outlive the call; `value` is a u32 and `size`
    // says so.
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            core::ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    if status != 0 {
        return None;
    }
    Some(if value == 0 {
        tpt_window_core::Theme::Dark
    } else {
        tpt_window_core::Theme::Light
    })
}

/// Encodes a theme for the atomic: 0 light, 1 dark, 2 unknown.
fn theme_bits(theme: Option<tpt_window_core::Theme>) -> u8 {
    match theme {
        Some(tpt_window_core::Theme::Light) => 0,
        Some(tpt_window_core::Theme::Dark) => 1,
        None => 2,
    }
}

fn bits_theme(bits: u8) -> Option<tpt_window_core::Theme> {
    match bits {
        0 => Some(tpt_window_core::Theme::Light),
        1 => Some(tpt_window_core::Theme::Dark),
        _ => None,
    }
}

/// The full bounds of the display a window is on, in desktop pixels.
fn monitor_bounds(hwnd: HWND) -> Option<RECT> {
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
    };
    // SAFETY: a plain query on a window handle this crate owns.
    let hmon = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if hmon.is_null() {
        return None;
    }
    let mut info: MONITORINFO = unsafe { core::mem::zeroed() };
    info.cbSize = size_of::<MONITORINFO>() as u32;
    // SAFETY: `info` is a MONITORINFO with its size set.
    if unsafe { GetMonitorInfoW(hmon, &mut info) } == 0 {
        return None;
    }
    Some(info.rcMonitor)
}

/// Lists the attached displays with their names, bounds, primary flag and effective DPI.
fn enumerate_monitors() -> Vec<tpt_window_core::MonitorInfo> {
    use windows_sys::Win32::Graphics::Gdi::{EnumDisplayMonitors, HDC};
    use windows_sys::core::BOOL;

    unsafe extern "system" fn collect(
        hmon: windows_sys::Win32::Graphics::Gdi::HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        // SAFETY: `lparam` is the `&mut Vec` passed to EnumDisplayMonitors below, and it is
        // only used during that call.
        let out = unsafe { &mut *(lparam as *mut Vec<tpt_window_core::MonitorInfo>) };
        if let Some(info) = monitor_info(hmon) {
            out.push(info);
        }
        1
    }

    let mut out: Vec<tpt_window_core::MonitorInfo> = Vec::new();
    // SAFETY: a null HDC and clip rect enumerate every display; `collect` only touches `out`
    // through the pointer given here, which outlives the call.
    unsafe {
        EnumDisplayMonitors(
            core::ptr::null_mut(),
            core::ptr::null(),
            Some(collect),
            (&mut out as *mut Vec<tpt_window_core::MonitorInfo>) as LPARAM,
        )
    };
    out
}

/// Wakes the loop thread by posting a thread message, which `MsgWaitForMultipleObjects` sees.
struct ThreadWake {
    thread_id: u32,
}

impl tpt_window_core::proxy::Wake for ThreadWake {
    fn wake(&self) {
        // A null message does nothing when dispatched; it only ends the wait.
        // SAFETY: posting to a thread id is valid; a stale id just fails and returns 0.
        unsafe { PostThreadMessageW(self.thread_id, WM_NULL, 0, 0) };
    }
}

/// Creates an [`EventLoop`] backed by Win32. Must be called on the thread that will
/// pump it.
pub fn new_event_loop() -> Result<EventLoop, WindowError> {
    // Best effort: fails harmlessly if the process already chose an awareness mode.
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    register_class()?;
    PENDING.with(|_| {}); // force the thread-local ring to allocate now, not on the hot path
    Ok(EventLoop::with_backend(Box::new(Win32EventLoop {
        exit: Arc::new(AtomicBool::new(false)),
        // SAFETY: plain query with no arguments.
        thread_id: unsafe { GetCurrentThreadId() },
    })))
}

impl Win32EventLoop {
    fn pump(&self) {
        let mut msg: MSG = unsafe { core::mem::zeroed() };
        while unsafe { PeekMessageW(&mut msg, null_mut(), 0, 0, PM_REMOVE) } != 0 {
            if msg.message == WM_QUIT {
                self.exit.store(true, Ordering::Release);
                break;
            }
            unsafe {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    }

    fn deliver(&self, sink: &mut EventSink<'_>) -> PollStatus {
        let delivered = PENDING.with(|p| sink.drain_pending(&mut p.borrow_mut()));
        if self.exit.load(Ordering::Acquire) {
            PollStatus::Exit
        } else if delivered {
            PollStatus::Events
        } else {
            PollStatus::Idle
        }
    }
}

impl EventLoopBackend for Win32EventLoop {
    fn poll(&mut self, sink: &mut EventSink<'_>) -> PollStatus {
        self.pump();
        self.deliver(sink)
    }

    fn wait(&mut self, sink: &mut EventSink<'_>, timeout: Duration) -> PollStatus {
        let have_pending = PENDING.with(|p| !p.borrow().is_empty());
        if !have_pending {
            // Round up: a 0.5 ms request must wait, not spin. Only a zero timeout means no wait.
            let ms = u32::try_from(timeout.as_nanos().div_ceil(1_000_000)).unwrap_or(INFINITE - 1);
            unsafe { MsgWaitForMultipleObjects(0, null(), 0, ms, QS_ALLINPUT) };
        }
        self.pump();
        self.deliver(sink)
    }

    fn request_exit(&self) {
        self.exit.store(true, Ordering::Release);
    }

    fn monitors(&self) -> Vec<tpt_window_core::MonitorInfo> {
        enumerate_monitors()
    }

    fn wake_handle(&self) -> Option<Arc<dyn tpt_window_core::proxy::Wake>> {
        Some(Arc::new(ThreadWake {
            thread_id: self.thread_id,
        }))
    }

    fn create_window(
        &self,
        attrs: &WindowAttributes,
    ) -> Result<Arc<dyn WindowBackend>, WindowError> {
        // The display the window will open on decides its size. With a position, that is the
        // display under the position; without one, the system's scale is used.
        let system_dpi = unsafe { GetDpiForSystem() };
        let (dpi, scale) = match attrs.position {
            Some(p) => {
                let approx = p.to_physical(f64::from(system_dpi) / 96.0);
                let dpi =
                    dpi_at_point(PhysicalPosition::new(approx.x, approx.y)).unwrap_or(system_dpi);
                (dpi, f64::from(dpi) / 96.0)
            }
            None => (system_dpi, f64::from(system_dpi) / 96.0),
        };

        let mut style = if attrs.decorations {
            WS_OVERLAPPEDWINDOW
        } else {
            WS_POPUP
        };
        if !attrs.resizable {
            style &= !(WS_THICKFRAME | WS_MAXIMIZEBOX);
        }
        let ex = WS_EX_APPWINDOW;

        let inner = attrs
            .inner_size
            .unwrap_or(LogicalSize::new(800.0, 600.0))
            .to_physical(scale);
        let (w, h) = outer_size(inner, style, ex, dpi);
        let (x, y) = match attrs.position {
            Some(p) => {
                let p = p.to_physical(scale);
                (p.x, p.y)
            }
            None => (CW_USEDEFAULT, CW_USEDEFAULT),
        };

        let data = Arc::new(WindowData {
            id: WindowId(NEXT_ID.fetch_add(1, Ordering::Relaxed)),
            state: SharedWindowState::new(WindowState {
                size: inner,
                position: PhysicalPosition::new(0, 0),
                scale_factor: scale,
                focused: false,
                visible: false,
            }),
            commands: Mutex::new(VecDeque::new()),
            limits: Mutex::new(Limits {
                min: attrs.min_size,
                max: attrs.max_size,
            }),
            cursor_inside: AtomicBool::new(false),
            cursor_visible: AtomicBool::new(true),
            buttons_down: AtomicU8::new(0),
            held_keys: Mutex::new(Vec::with_capacity(16)),
            held_buttons: Mutex::new(Vec::with_capacity(5)),
            minimized: AtomicBool::new(false),
            fullscreen_restore: Mutex::new(None),
            modifiers: AtomicU8::new(0),
            theme: AtomicU8::new(theme_bits(system_theme())),
            grab: AtomicU8::new(0),
            icon: AtomicIsize::new(0),
            high_surrogate: AtomicU16::new(0),
            last_w: AtomicI32::new(0),
            last_h: AtomicI32::new(0),
        });

        let title: Vec<u16> = attrs.title.encode_utf16().chain(Some(0)).collect();
        let raw = Arc::into_raw(data.clone());
        UNCLAIMED_REF.with(|c| c.set(raw as usize));
        let hwnd = unsafe {
            CreateWindowExW(
                ex,
                CLASS_NAME.as_ptr(),
                title.as_ptr(),
                style,
                x,
                y,
                w,
                h,
                null_mut(),
                null_mut(),
                hinstance(),
                raw as *const core::ffi::c_void,
            )
        };
        if !hwnd.is_null() {
            // Ask for WM_TOUCH from this window's touch input.
            unsafe { RegisterTouchWindow(hwnd, 0) };
            // Files dropped from Explorer arrive as WM_DROPFILES.
            unsafe { DragAcceptFiles(hwnd, 1) };
            // Raw mouse input goes to one window per process: the last one created.
            let mouse = RAWINPUTDEVICE {
                usUsagePage: 0x01,
                usUsage: 0x02,
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            };
            unsafe {
                RegisterRawInputDevices(&mouse, 1, size_of::<RAWINPUTDEVICE>() as u32);
            }
            RAW_MOUSE_OWNER.with(|c| c.set(hwnd as usize));
        }
        if hwnd.is_null() {
            // If WM_NCCREATE ran, the window took the reference and WM_NCDESTROY, which the
            // system sends on a failed create, has already released it. Reclaim it only when
            // nothing took it, so it is never released twice.
            if UNCLAIMED_REF.with(|c| c.get()) == raw as usize {
                drop(unsafe { Arc::from_raw(raw) });
            }
            UNCLAIMED_REF.with(|c| c.set(0));
            return Err(WindowError::Os("CreateWindowExW failed".into()));
        }
        UNCLAIMED_REF.with(|c| c.set(0));

        let real_dpi = unsafe { GetDpiForWindow(hwnd) };
        let size = client_size(hwnd);
        let pos = outer_pos(hwnd);
        data.state.update(|s| {
            s.scale_factor = f64::from(real_dpi) / 96.0;
            s.size = size;
            s.position = pos;
            s.visible = attrs.visible;
        });
        if attrs.visible {
            unsafe { ShowWindow(hwnd, SW_SHOW) };
        }
        Ok(Arc::new(Win32Window {
            hwnd: AtomicIsize::new(hwnd as isize),
            data,
        }))
    }
}

/// A Win32 window.
pub struct Win32Window {
    hwnd: AtomicIsize,
    data: Arc<WindowData>,
}

impl Win32Window {
    fn hwnd(&self) -> HWND {
        self.hwnd.load(Ordering::Relaxed) as HWND
    }
}

impl WindowBackend for Win32Window {
    fn id(&self) -> WindowId {
        self.data.id
    }

    fn state(&self) -> &Arc<SharedWindowState> {
        &self.data.state
    }

    fn theme(&self) -> Option<tpt_window_core::Theme> {
        bits_theme(self.data.theme.load(Ordering::Relaxed))
    }

    fn current_monitor(&self) -> Option<tpt_window_core::MonitorInfo> {
        let hwnd = self.hwnd.load(Ordering::Relaxed);
        if hwnd == 0 {
            return None;
        }
        window_monitor(hwnd as HWND)
    }

    fn raw_window_handle(&self) -> Result<RawWindowHandle, WindowError> {
        let hwnd =
            NonZeroIsize::new(self.hwnd.load(Ordering::Relaxed)).ok_or(WindowError::Destroyed)?;
        let mut h = Win32WindowHandle::new(hwnd);
        h.hinstance = NonZeroIsize::new(hinstance() as isize);
        Ok(RawWindowHandle::Win32(h))
    }

    fn raw_display_handle(&self) -> Result<RawDisplayHandle, WindowError> {
        Ok(RawDisplayHandle::Windows(WindowsDisplayHandle::new()))
    }

    fn send_command(&self, command: WindowCommand) {
        self.data
            .commands
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(command);
        // PostMessage is thread-safe and also wakes a blocked `step`.
        unsafe { PostMessageW(self.hwnd(), WM_TPT_COMMAND, 0, 0) };
    }
}

impl Drop for Win32Window {
    fn drop(&mut self) {
        // DestroyWindow must run on the owning thread; ask it to.
        unsafe { PostMessageW(self.hwnd(), WM_TPT_DESTROY, 0, 0) };
    }
}

#[cfg(test)]
mod composition_tests {
    use super::{StackText, decode_units};

    #[test]
    fn composition_units_decode_to_utf8() {
        let mut out = StackText::new();
        // "a", then U+1F600 as a surrogate pair, then a lone surrogate that becomes U+FFFD.
        decode_units(&[0x61, 0xD83D, 0xDE00, 0xD800], &mut out);
        assert_eq!(out.as_str(), "a\u{1F600}\u{FFFD}");
    }

    #[test]
    fn overlong_composition_is_cut_whole_characters_only() {
        let mut out = StackText::new();
        let units = vec![0x00E9_u16; 400];
        decode_units(&units, &mut out);
        assert!(out.as_str().chars().all(|c| c == '\u{e9}'));
        assert!(out.as_str().len() <= 1024);
    }
}

#[cfg(test)]
mod wm_char_tests {
    use super::wm_char_to_char;

    #[test]
    fn plain_bmp_unit_is_its_character() {
        assert_eq!(wm_char_to_char(0, u16::from(b'a')), Some('a'));
        assert_eq!(wm_char_to_char(0, 0x00E9), Some('é'));
    }

    #[test]
    fn surrogate_pair_joins_into_one_character() {
        // U+1F600 is D83D DE00.
        assert_eq!(wm_char_to_char(0xD83D, 0xDE00), Some('\u{1F600}'));
    }

    #[test]
    fn lone_low_surrogate_is_dropped_not_nul() {
        assert_eq!(wm_char_to_char(0, 0xDE00), None);
    }
}
