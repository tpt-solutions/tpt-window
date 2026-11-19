// SPDX-License-Identifier: MIT OR Apache-2.0
//! Win32 backend for tpt-window (wraps `windows-sys`, the raw-FFI crate from the
//! windows-rs project; MIT OR Apache-2.0).
//!
//! Design notes:
//! * The window procedure runs inside `poll`/`step` (it is dispatched by our own message
//!   pump), translates messages into [`Event`]s and pushes them into a pre-allocated
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
use std::sync::{Arc, Mutex, Once};
use std::time::Duration;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, Win32WindowHandle, WindowsDisplayHandle,
};
use tpt_window_core::*;
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{InvalidateRect, ValidateRect};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::System::Threading::INFINITE;
use windows_sys::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem,
    GetDpiForWindow, SetProcessDpiAwarenessContext,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
    ReleaseCapture, SetCapture, TME_LEAVE, TRACKMOUSEEVENT, TrackMouseEvent,
};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

/// Not in the enabled windows-sys feature set; value from winuser.h.
const WM_MOUSELEAVE: u32 = 0x02A3;
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
    static PENDING: RefCell<RingBuffer<Event>> =
        RefCell::new(RingBuffer::with_capacity(4096).with_policy(OverflowPolicy::DropOldest));
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
    buttons_down: AtomicU8,
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
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, data as isize) };
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let Some(d) = (unsafe { data_of(hwnd) }) else {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    };
    let id = d.id;

    match msg {
        WM_NCDESTROY => {
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
            if wparam == SIZE_MINIMIZED as usize {
                push_event(id, WindowEvent::Occluded(true));
            } else {
                if d.last_w.load(Ordering::Relaxed) == 0 && d.last_h.load(Ordering::Relaxed) == 0 {
                    // first real size
                }
                push_event(id, WindowEvent::Occluded(false));
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
            let pos = outer_pos(hwnd);
            d.state.update(|s| s.position = pos);
            push_event(id, WindowEvent::Moved(pos));
            return 0;
        }
        WM_SETFOCUS | WM_KILLFOCUS => {
            let f = msg == WM_SETFOCUS;
            d.state.update(|s| s.focused = f);
            push_event(id, WindowEvent::Focused(f));
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
                        if d.buttons_down.fetch_add(1, Ordering::Relaxed) == 0 {
                            unsafe { SetCapture(hwnd) };
                        }
                    }
                    ElementState::Released => {
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
            // Skip control characters (Backspace, Enter, Tab, Escape arrive as keys).
            if unit >= 0x20 && unit != 0x7F {
                let ch = if (0xD800..0xDC00).contains(&unit) {
                    d.high_surrogate.store(unit, Ordering::Relaxed);
                    None
                } else if (0xDC00..0xE000).contains(&unit) {
                    let hi = d.high_surrogate.swap(0, Ordering::Relaxed);
                    char::decode_utf16([hi, unit]).next().and_then(Result::ok)
                } else {
                    char::from_u32(u32::from(unit))
                };
                if let Some(ch) = ch {
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
        WindowCommand::RequestRedraw => {
            unsafe { InvalidateRect(hwnd, null(), 0) };
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

static REGISTER: Once = Once::new();
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

fn hinstance() -> *mut core::ffi::c_void {
    unsafe { GetModuleHandleW(null()) }
}

fn register_class() -> Result<(), WindowError> {
    let mut result = Ok(());
    REGISTER.call_once(|| {
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
    });
    result
}

/// Win32 event loop backend. Create with [`new_event_loop`].
pub struct Win32EventLoop {
    exit: Arc<AtomicBool>,
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
        let mut delivered = false;
        while let Some(ev) = PENDING.with(|p| p.borrow().front()) {
            match sink.push(ev) {
                EventStatus::Accepted => {
                    PENDING.with(|p| p.borrow_mut().pop());
                    delivered = true;
                }
                EventStatus::BufferFull | EventStatus::Captured => break,
            }
        }
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
            let ms = u32::try_from(timeout.as_millis()).unwrap_or(INFINITE - 1);
            unsafe { MsgWaitForMultipleObjects(0, null(), 0, ms, QS_ALLINPUT) };
        }
        self.pump();
        self.deliver(sink)
    }

    fn request_exit(&self) {
        self.exit.store(true, Ordering::Release);
    }

    fn create_window(
        &self,
        attrs: &WindowAttributes,
    ) -> Result<Arc<dyn WindowBackend>, WindowError> {
        let dpi = unsafe { GetDpiForSystem() };
        let scale = f64::from(dpi) / 96.0;

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
            buttons_down: AtomicU8::new(0),
            high_surrogate: AtomicU16::new(0),
            last_w: AtomicI32::new(0),
            last_h: AtomicI32::new(0),
        });

        let title: Vec<u16> = attrs.title.encode_utf16().chain(Some(0)).collect();
        let raw = Arc::into_raw(data.clone());
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
        if hwnd.is_null() {
            // SAFETY: WM_NCCREATE may not have run, so reclaim the reference we leaked.
            // If it did run, WM_NCDESTROY already released it (hwnd would be non-null then).
            drop(unsafe { Arc::from_raw(raw) });
            return Err(WindowError::Os("CreateWindowExW failed".into()));
        }

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
