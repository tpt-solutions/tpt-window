// SPDX-License-Identifier: MIT OR Apache-2.0
//! macOS backend for tpt-window, built on AppKit through `objc2`.
//!
//! Design notes:
//! - Everything AppKit runs on the thread that created the `EventLoop`
//!   (`MainThreadMarker`). Other threads only queue commands, which are applied
//!   on the next poll.
//! - Input is read by pumping `NSEvent`s and translating them, then forwarding each
//!   event to `NSApplication::sendEvent` so AppKit keeps its normal behaviour. The
//!   one custom `NSView` subclass only reports `drawRect:` as `RedrawRequested`.
//! - A window delegate vetoes the close button. The backend reports `CloseRequested`
//!   and keeps the window open, so the application decides by dropping its window.
//!   This matches Win32 and Wayland. Compile-checked for macOS only; not yet run on a Mac.
//! - Cursor enter/leave come from a tracking area on the content view, covering the
//!   whole view (`InVisibleRect`), and are translated from `MouseEntered`/`MouseExited`.
//!
//! Only the macOS build is compiled; on other platforms this crate is empty.
#![cfg(target_os = "macos")]
#![warn(missing_docs)]

use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::runtime::ProtocolObject;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSCursor, NSEvent,
    NSEventMask, NSEventModifierFlags, NSEventType, NSFloatingWindowLevel, NSMenu, NSMenuItem,
    NSNormalWindowLevel, NSScreen, NSTrackingArea, NSTrackingAreaOptions, NSView, NSWindow,
    NSWindowDelegate, NSWindowStyleMask,
};
use objc2_foundation::{
    NSDate, NSDefaultRunLoopMode, NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize, NSString,
};
use raw_window_handle::{
    AppKitDisplayHandle, AppKitWindowHandle, RawDisplayHandle, RawWindowHandle,
};
use tpt_window_core::*;

// ---------------------------------------------------------------------------
// Thread-local state (main thread only)
// ---------------------------------------------------------------------------

thread_local! {
    /// Events waiting for the sink. Filled while AppKit events are translated.
    static PENDING: RefCell<EventQueue> = RefCell::new(new_pending_queue());

    /// Every live window on this thread, with the AppKit object that backs it.
    static WINDOWS: RefCell<Vec<Record>> = const { RefCell::new(Vec::new()) };
}

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Whether `c` is in the Unicode Private Use Area. AppKit reports arrow, F and navigation
/// keys as characters from it, so they must not be committed as text.
/// Whether this process has hidden the pointer. The pointer is one global resource on macOS,
/// so the state is shared by every window.
static CURSOR_HIDDEN: AtomicBool = AtomicBool::new(false);

fn is_private_use(c: char) -> bool {
    ('\u{E000}'..='\u{F8FF}').contains(&c)
}

fn push_event(window: WindowId, event: WindowEvent) {
    PENDING.with(|p| {
        p.borrow_mut().push(Event { window, event });
    });
}

struct Record {
    ns_window: Retained<NSWindow>,
    /// The window holds its delegate weakly, so the record must keep it alive.
    _delegate: Retained<WindowDelegate>,
    data: Arc<WindowData>,
}

define_class!(
    // Turns the close button into `CloseRequested` and keeps the window open. The
    // application closes it by dropping its window, which the poll loop then does.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "TptWindowDelegate"]
    #[ivars = Cell<u64>]
    struct WindowDelegate;

    unsafe impl NSObjectProtocol for WindowDelegate {}

    unsafe impl NSWindowDelegate for WindowDelegate {
        #[unsafe(method(windowShouldClose:))]
        fn window_should_close(&self, _sender: &NSWindow) -> bool {
            push_event(WindowId(self.ivars().get()), WindowEvent::CloseRequested);
            false
        }
    }
);

impl WindowDelegate {
    fn new(mtm: MainThreadMarker, id: WindowId) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Cell::new(id.0));
        unsafe { msg_send![super(this), init] }
    }
}

define_class!(
    // The content view. AppKit calls `drawRect:` when the view needs painting (after a
    // `setNeedsDisplay` or an exposure), and that is reported as `RedrawRequested`.
    #[unsafe(super(NSView))]
    #[thread_kind = MainThreadOnly]
    #[name = "TptContentView"]
    #[ivars = Cell<u64>]
    struct ContentView;

    impl ContentView {
        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _dirty: NSRect) {
            push_event(WindowId(self.ivars().get()), WindowEvent::RedrawRequested);
        }

        // Keys reach the view through the responder chain. Handling them here, as nothing,
        // stops AppKit from playing the error beep for keys no one handles.
        #[unsafe(method(keyDown:))]
        fn key_down(&self, _event: &NSEvent) {}
    }
);

define_class!(
    // The target of the Quit menu item. Quit asks every window to close, which is reported as
    // `CloseRequested`; the application then decides, as for the close button. It does not call
    // `terminate:`, because that would end the process behind the application's back.
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "TptMenuTarget"]
    #[ivars = ()]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(quit:))]
        fn quit(&self, _sender: Option<&AnyObject>) {
            WINDOWS.with(|w| {
                for record in w.borrow().iter() {
                    if !record.data.destroy_requested.load(Ordering::Acquire) {
                        push_event(record.data.id, WindowEvent::CloseRequested);
                    }
                }
            });
        }
    }
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

impl ContentView {
    /// Creates a content view covering `frame`, reporting for window `id`.
    fn new(mtm: MainThreadMarker, id: WindowId, frame: NSRect) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Cell::new(id.0));
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

/// State shared between the main-thread record and the `Window` handle.
struct WindowData {
    id: WindowId,
    state: Arc<SharedWindowState>,
    commands: Mutex<VecDeque<WindowCommand>>,
    /// Address of the content `NSView`, or 0 once the window is destroyed.
    view: AtomicUsize,
    /// Set when the last handle is dropped; the poll loop then destroys the window.
    destroy_requested: AtomicBool,
}

// ---------------------------------------------------------------------------
// Coordinates
// ---------------------------------------------------------------------------

/// Describes one `NSScreen` in physical pixels, with a top-left origin. `main_height` is the
/// primary display's height in points, which AppKit's bottom-left origin is measured from.
fn screen_info(screen: &NSScreen, main_height: f64, primary: bool) -> tpt_window_core::MonitorInfo {
    let frame = screen.frame();
    let scale = screen.backingScaleFactor();
    let top = main_height - (frame.origin.y + frame.size.height);
    tpt_window_core::MonitorInfo {
        name: Some(screen.localizedName().to_string()),
        position: PhysicalPosition::new(
            (frame.origin.x * scale).round() as i32,
            (top * scale).round() as i32,
        ),
        size: PhysicalSize::new(
            (frame.size.width * scale).round() as u32,
            (frame.size.height * scale).round() as u32,
        ),
        scale_factor: scale,
        primary,
    }
}

/// Height of the main screen in points, used to convert AppKit's bottom-left origin
/// into the top-left origin tpt-window uses.
fn screen_height(mtm: MainThreadMarker) -> f64 {
    NSScreen::mainScreen(mtm).map_or(0.0, |s| s.frame().size.height)
}

fn content_size_points(window: &NSWindow) -> (f64, f64) {
    window
        .contentView()
        .map(|v| {
            let f = v.frame();
            (f.size.width, f.size.height)
        })
        .unwrap_or((0.0, 0.0))
}

/// Physical size of the content area.
fn physical_size(window: &NSWindow) -> PhysicalSize<u32> {
    let scale = window.backingScaleFactor();
    let (w, h) = content_size_points(window);
    PhysicalSize::new(
        (w * scale).round().max(0.0) as u32,
        (h * scale).round().max(0.0) as u32,
    )
}

/// Physical top-left position of the window frame.
fn physical_position(window: &NSWindow, mtm: MainThreadMarker) -> PhysicalPosition<i32> {
    let scale = window.backingScaleFactor();
    let frame = window.frame();
    let top = screen_height(mtm) - (frame.origin.y + frame.size.height);
    PhysicalPosition::new(
        (frame.origin.x * scale).round() as i32,
        (top * scale).round() as i32,
    )
}

// ---------------------------------------------------------------------------
// Keyboard mapping (kVK_* virtual key codes, which name physical key positions)
// ---------------------------------------------------------------------------

/// Maps a macOS virtual key code to a [`KeyCode`]. Codes name physical positions, so
/// the result does not depend on the keyboard layout.
/// The layout meaning of a key event: the character the key types with Shift and the other
/// modifiers ignored, when that is a key. Keys that type no character (arrows, function keys) and
/// keys the character table does not know use the key's position, which is the same on every
/// layout for those keys.
fn base_key(event: &NSEvent, vk: u16) -> KeyCode {
    if let Some(chars) = event.charactersIgnoringModifiers() {
        if let Some(c) = chars.to_string().chars().next() {
            if let Some(key) = tpt_window_core::keymap::key_from_char(c) {
                return key;
            }
        }
    }
    key_from_vk(vk)
}

fn key_from_vk(vk: u16) -> KeyCode {
    match vk {
        0x00 => KeyCode::A,
        0x01 => KeyCode::S,
        0x02 => KeyCode::D,
        0x03 => KeyCode::F,
        0x04 => KeyCode::H,
        0x05 => KeyCode::G,
        0x06 => KeyCode::Z,
        0x07 => KeyCode::X,
        0x08 => KeyCode::C,
        0x09 => KeyCode::V,
        0x0B => KeyCode::B,
        0x0C => KeyCode::Q,
        0x0D => KeyCode::W,
        0x0E => KeyCode::E,
        0x0F => KeyCode::R,
        0x10 => KeyCode::Y,
        0x11 => KeyCode::T,
        0x12 => KeyCode::Digit1,
        0x13 => KeyCode::Digit2,
        0x14 => KeyCode::Digit3,
        0x15 => KeyCode::Digit4,
        0x16 => KeyCode::Digit6,
        0x17 => KeyCode::Digit5,
        0x18 => KeyCode::Equal,
        0x19 => KeyCode::Digit9,
        0x1A => KeyCode::Digit7,
        0x1B => KeyCode::Minus,
        0x1C => KeyCode::Digit8,
        0x1D => KeyCode::Digit0,
        0x1E => KeyCode::BracketRight,
        0x1F => KeyCode::O,
        0x20 => KeyCode::U,
        0x21 => KeyCode::BracketLeft,
        0x22 => KeyCode::I,
        0x23 => KeyCode::P,
        0x24 => KeyCode::Enter,
        0x25 => KeyCode::L,
        0x26 => KeyCode::J,
        0x27 => KeyCode::Quote,
        0x28 => KeyCode::K,
        0x29 => KeyCode::Semicolon,
        0x2A => KeyCode::Backslash,
        0x2B => KeyCode::Comma,
        0x2C => KeyCode::Slash,
        0x2D => KeyCode::N,
        0x2E => KeyCode::M,
        0x2F => KeyCode::Period,
        0x30 => KeyCode::Tab,
        0x31 => KeyCode::Space,
        0x32 => KeyCode::Backquote,
        0x33 => KeyCode::Backspace,
        0x35 => KeyCode::Escape,
        0x36 => KeyCode::SuperRight,
        0x37 => KeyCode::SuperLeft,
        0x38 => KeyCode::ShiftLeft,
        0x39 => KeyCode::CapsLock,
        0x3A => KeyCode::AltLeft,
        0x3B => KeyCode::ControlLeft,
        0x3C => KeyCode::ShiftRight,
        0x3D => KeyCode::AltRight,
        0x3E => KeyCode::ControlRight,
        0x41 => KeyCode::NumpadDecimal,
        0x43 => KeyCode::NumpadMultiply,
        0x45 => KeyCode::NumpadAdd,
        0x4B => KeyCode::NumpadDivide,
        0x4C => KeyCode::NumpadEnter,
        0x4E => KeyCode::NumpadSubtract,
        0x52 => KeyCode::Numpad0,
        0x53 => KeyCode::Numpad1,
        0x54 => KeyCode::Numpad2,
        0x55 => KeyCode::Numpad3,
        0x56 => KeyCode::Numpad4,
        0x57 => KeyCode::Numpad5,
        0x58 => KeyCode::Numpad6,
        0x59 => KeyCode::Numpad7,
        0x5B => KeyCode::Numpad8,
        0x5C => KeyCode::Numpad9,
        0x60 => KeyCode::F5,
        0x61 => KeyCode::F6,
        0x62 => KeyCode::F7,
        0x63 => KeyCode::F3,
        0x64 => KeyCode::F8,
        0x65 => KeyCode::F9,
        0x67 => KeyCode::F11,
        0x6D => KeyCode::F10,
        0x6F => KeyCode::F12,
        0x72 => KeyCode::Insert,
        0x73 => KeyCode::Home,
        0x74 => KeyCode::PageUp,
        0x75 => KeyCode::Delete,
        0x76 => KeyCode::F4,
        0x77 => KeyCode::End,
        0x78 => KeyCode::F2,
        0x79 => KeyCode::PageDown,
        0x7A => KeyCode::F1,
        0x7B => KeyCode::ArrowLeft,
        0x7C => KeyCode::ArrowRight,
        0x7D => KeyCode::ArrowDown,
        0x7E => KeyCode::ArrowUp,
        _ => KeyCode::Unidentified,
    }
}

/// Whether a virtual key code is a modifier, and which device-dependent flag bit
/// reports it (from IOKit's NX_DEVICE* masks).
fn modifier_device_bit(vk: u16) -> Option<u64> {
    match vk {
        0x38 => Some(0x0000_0002), // left shift
        0x3C => Some(0x0000_0004), // right shift
        0x3B => Some(0x0000_0001), // left control
        0x3E => Some(0x0000_2000), // right control
        0x3A => Some(0x0000_0020), // left option
        0x3D => Some(0x0000_0040), // right option
        0x37 => Some(0x0000_0008), // left command
        0x36 => Some(0x0000_0010), // right command
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// NSEvent translation (main thread)
// ---------------------------------------------------------------------------

fn mouse_button(button_number: isize) -> MouseButton {
    match button_number {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        n => MouseButton::Other(n.clamp(0, i64::from(u16::MAX) as isize) as u16),
    }
}

/// Queues `Commit` events for `text`, split into chunks that fit an [`ImeText`].
fn push_text(id: WindowId, mut text: &str) {
    while !text.is_empty() {
        let (chunk, rest) = ImeText::take_chunk(text);
        let continued = !rest.is_empty();
        push_event(
            id,
            WindowEvent::Ime(ImeEvent::Commit {
                text: chunk,
                continued,
            }),
        );
        text = rest;
    }
}

/// Translates one AppKit event for `record` and queues the resulting events.
fn translate(record: &Record, event: &NSEvent, mtm: MainThreadMarker) {
    let id = record.data.id;
    let window = &record.ns_window;
    let scale = window.backingScaleFactor();
    let ty = event.r#type();

    let pointer = |event: &NSEvent| -> PhysicalPosition<f64> {
        let p = event.locationInWindow();
        let (_, h) = content_size_points(window);
        // AppKit's origin is bottom-left; tpt-window's is top-left.
        PhysicalPosition::new(p.x * scale, (h - p.y) * scale)
    };

    match ty {
        NSEventType::LeftMouseDown | NSEventType::RightMouseDown | NSEventType::OtherMouseDown => {
            let button = match ty {
                NSEventType::LeftMouseDown => MouseButton::Left,
                NSEventType::RightMouseDown => MouseButton::Right,
                _ => mouse_button(event.buttonNumber()),
            };
            push_event(
                id,
                WindowEvent::MouseInput {
                    state: ElementState::Pressed,
                    button,
                },
            );
        }
        NSEventType::LeftMouseUp | NSEventType::RightMouseUp | NSEventType::OtherMouseUp => {
            let button = match ty {
                NSEventType::LeftMouseUp => MouseButton::Left,
                NSEventType::RightMouseUp => MouseButton::Right,
                _ => mouse_button(event.buttonNumber()),
            };
            push_event(
                id,
                WindowEvent::MouseInput {
                    state: ElementState::Released,
                    button,
                },
            );
        }
        NSEventType::MouseEntered => push_event(id, WindowEvent::CursorEntered),
        NSEventType::MouseExited => push_event(id, WindowEvent::CursorLeft),
        NSEventType::MouseMoved
        | NSEventType::LeftMouseDragged
        | NSEventType::RightMouseDragged
        | NSEventType::OtherMouseDragged => {
            push_event(
                id,
                WindowEvent::CursorMoved {
                    position: pointer(event),
                    pressure: 0.0,
                },
            );
        }
        NSEventType::ScrollWheel => {
            let delta = if event.hasPreciseScrollingDeltas() {
                MouseScrollDelta::PixelDelta {
                    x: event.scrollingDeltaX() * scale,
                    y: event.scrollingDeltaY() * scale,
                }
            } else {
                MouseScrollDelta::LineDelta {
                    x: event.scrollingDeltaX() as f32,
                    y: event.scrollingDeltaY() as f32,
                }
            };
            push_event(
                id,
                WindowEvent::MouseWheel {
                    delta,
                    phase: TouchPhase::Moved,
                },
            );
        }
        NSEventType::KeyDown => {
            let vk = event.keyCode();
            let physical = key_from_vk(vk);
            push_event(
                id,
                WindowEvent::KeyboardInput {
                    state: ElementState::Pressed,
                    key_code: base_key(event, vk),
                    physical_key: PhysicalKey::Code(physical),
                },
            );
            // Command shortcuts (Cmd+C, Cmd+Q) are not text input.
            let command_held = event.modifierFlags().0 & NSEventModifierFlags::Command.0 != 0;
            if let Some(chars) = event.characters().filter(|_| !command_held) {
                let text = chars.to_string();
                // Control characters (arrows, Escape, Return) and the Private Use Area that
                // AppKit uses for function keys (U+F700 and up) are not text.
                if text.chars().all(|c| !c.is_control() && !is_private_use(c)) {
                    push_text(id, &text);
                }
            }
        }
        NSEventType::KeyUp => {
            let vk = event.keyCode();
            push_event(
                id,
                WindowEvent::KeyboardInput {
                    state: ElementState::Released,
                    key_code: base_key(event, vk),
                    physical_key: PhysicalKey::Code(key_from_vk(vk)),
                },
            );
        }
        NSEventType::FlagsChanged => {
            // AppKit sends this only when the modifier flags change, so every one is a change.
            let flags = event.modifierFlags();
            let held = |f: NSEventModifierFlags| flags.0 & f.0 != 0;
            push_event(
                id,
                WindowEvent::ModifiersChanged(ModifiersState::new(
                    held(NSEventModifierFlags::Shift),
                    held(NSEventModifierFlags::Control),
                    held(NSEventModifierFlags::Option),
                    held(NSEventModifierFlags::Command),
                )),
            );
            let vk = event.keyCode();
            if let Some(bit) = modifier_device_bit(vk) {
                let flags: NSEventModifierFlags = event.modifierFlags();
                let pressed = (flags.0 as u64) & bit != 0;
                let code = key_from_vk(vk);
                push_event(
                    id,
                    WindowEvent::KeyboardInput {
                        state: if pressed {
                            ElementState::Pressed
                        } else {
                            ElementState::Released
                        },
                        key_code: code,
                        physical_key: PhysicalKey::Code(code),
                    },
                );
            }
        }
        _ => {}
    }
    // Keep mtm in the signature: translation is only valid on the main thread.
    let _ = mtm;
}

// ---------------------------------------------------------------------------
// Window bookkeeping (main thread)
// ---------------------------------------------------------------------------

/// Applies queued commands and reports window state changes for one record.
fn sync_record(record: &Record, mtm: MainThreadMarker) {
    let window = &record.ns_window;
    let data = &record.data;

    // Commands from handles.
    let commands: Vec<WindowCommand> = {
        let mut q = data.commands.lock().unwrap_or_else(|e| e.into_inner());
        q.drain(..).collect()
    };
    for cmd in commands {
        match cmd {
            WindowCommand::SetTitle(t) => window.setTitle(&NSString::from_str(&t)),
            WindowCommand::SetSize(size) => {
                let scale = window.backingScaleFactor();
                window.setContentSize(NSSize::new(
                    f64::from(size.width) / scale,
                    f64::from(size.height) / scale,
                ));
            }
            WindowCommand::SetPosition(p) => {
                let scale = window.backingScaleFactor();
                let top = f64::from(p.y) / scale;
                let x = f64::from(p.x) / scale;
                window.setFrameTopLeftPoint(NSPoint::new(x, screen_height(mtm) - top));
            }
            WindowCommand::SetVisible(v) => {
                if v {
                    window.orderFront(None);
                } else {
                    window.orderOut(None);
                }
                data.state.update(|s| s.visible = v);
            }
            WindowCommand::RequestRedraw => {
                if let Some(view) = window.contentView() {
                    view.setNeedsDisplay(true);
                }
            }
            // Content-size limits, so they match the physical sizes the caller passes.
            WindowCommand::SetMinSize(m) => {
                let scale = window.backingScaleFactor();
                let (w, h) = m.map_or((0.0, 0.0), |m| {
                    (f64::from(m.width) / scale, f64::from(m.height) / scale)
                });
                window.setContentMinSize(NSSize::new(w, h));
            }
            WindowCommand::SetMaxSize(m) => {
                let scale = window.backingScaleFactor();
                let (w, h) = m.map_or((f64::MAX, f64::MAX), |m| {
                    (f64::from(m.width) / scale, f64::from(m.height) / scale)
                });
                window.setContentMaxSize(NSSize::new(w, h));
            }
            WindowCommand::SetResizable(r) => {
                // Borderless windows have no resize edge, so the mask is only changed with a title bar.
                let mut style = window.styleMask();
                if style.0 & NSWindowStyleMask::Titled.0 != 0 {
                    if r {
                        style |= NSWindowStyleMask::Resizable;
                    } else {
                        style &= !NSWindowStyleMask::Resizable;
                    }
                    window.setStyleMask(style);
                }
            }
            WindowCommand::SetDecorations(on) => {
                let titled = NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Miniaturizable;
                let mut style = window.styleMask();
                if on {
                    style |= titled;
                } else {
                    style &= !(titled | NSWindowStyleMask::Resizable);
                }
                window.setStyleMask(style);
            }
            WindowCommand::SetFullscreen(on) => {
                // `toggleFullScreen` toggles, so it is called only when the state differs.
                let now = window.styleMask().0 & NSWindowStyleMask::FullScreen.0 != 0;
                if now != on {
                    window.toggleFullScreen(None);
                }
            }
            WindowCommand::SetAlwaysOnTop(on) => {
                let level = if on {
                    NSFloatingWindowLevel
                } else {
                    NSNormalWindowLevel
                };
                window.setLevel(level);
            }
            // Not yet supported on macOS; the commands are accepted and have no effect.
            WindowCommand::SetCursorGrab(_) | WindowCommand::SetIcon(_) => {}
            WindowCommand::SetCursorVisible(visible) => {
                // NSCursor hide and unhide nest, so only a real change is sent.
                if CURSOR_HIDDEN.swap(!visible, Ordering::Relaxed) == visible {
                    if visible {
                        NSCursor::unhide();
                    } else {
                        NSCursor::hide();
                    }
                }
            }
            WindowCommand::SetMaximized(on) => {
                // `zoom` toggles, so only call it when the state differs.
                if window.isZoomed() != on {
                    window.zoom(None);
                }
            }
            WindowCommand::SetMinimized(on) => {
                if on {
                    window.miniaturize(None);
                } else if window.isMiniaturized() {
                    window.deminiaturize(None);
                }
            }
            // Not supported on macOS; the command is accepted and has no effect.
            WindowCommand::SetImeCursorArea { .. } => {}
            _ => debug_assert!(
                false,
                "unhandled WindowCommand: give it an arm in this backend"
            ),
        }
    }

    // Closed without the delegate's veto (the close button is vetoed, so this is a
    // programmatic close): the window is no longer visible and the app is not hidden.
    let app_hidden = NSApplication::sharedApplication(mtm).isHidden();
    // A minimized window is not visible either, so it is excluded explicitly.
    if data.state.snapshot().visible
        && !window.isVisible()
        && !window.isMiniaturized()
        && !app_hidden
    {
        push_event(data.id, WindowEvent::CloseRequested);
        // Let `reap_windows` close the window and report `Destroyed`.
        data.destroy_requested.store(true, Ordering::Release);
        return;
    }

    // State diffs.
    let prev = data.state.snapshot();
    let size = physical_size(window);
    let pos = physical_position(window, mtm);
    let scale = window.backingScaleFactor();
    let focused = window.isKeyWindow();
    let visible = window.isVisible();

    if size != prev.size {
        push_event(data.id, WindowEvent::Resized(size));
    }
    if pos != prev.position {
        push_event(data.id, WindowEvent::Moved(pos));
    }
    if (scale - prev.scale_factor).abs() > f64::EPSILON {
        push_event(
            data.id,
            WindowEvent::ScaleFactorChanged {
                scale_factor: scale,
                new_inner_size: size,
            },
        );
    }
    if focused != prev.focused {
        push_event(data.id, WindowEvent::Focused(focused));
    }
    data.state.update(|s| {
        s.size = size;
        s.position = pos;
        s.scale_factor = scale;
        s.focused = focused;
        s.visible = visible;
    });
}

/// Destroys windows whose last handle was dropped, and drops records of closed ones.
fn reap_windows(mtm: MainThreadMarker) {
    let _ = mtm;
    WINDOWS.with(|w| {
        let mut windows = w.borrow_mut();
        windows.retain(|record| {
            let gone = record.data.destroy_requested.load(Ordering::Acquire);
            if gone {
                record.ns_window.orderOut(None);
                record.ns_window.close();
                record.data.view.store(0, Ordering::Release);
                push_event(record.data.id, WindowEvent::Destroyed);
            }
            !gone
        });
    });
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

/// The macOS event loop.
pub struct MacEventLoop {
    exit: Arc<AtomicBool>,
    mtm: MainThreadMarker,
    /// Keeps the Quit item's target alive; the menu holds it weakly.
    _menu_target: Retained<MenuTarget>,
}

/// Creates an [`EventLoop`] backed by AppKit. Must be called on the main thread,
/// which then pumps it.
pub fn new_event_loop() -> Result<EventLoop, WindowError> {
    let mtm = MainThreadMarker::new().ok_or_else(|| {
        WindowError::Os("the macOS event loop must be created on the main thread".into())
    })?;
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
    app.finishLaunching();
    // A main menu with Quit (Cmd+Q). Without one there is no Quit at all.
    let menu_target = MenuTarget::new(mtm);
    let main_menu = NSMenu::new(mtm);
    let app_item = NSMenuItem::new(mtm);
    main_menu.addItem(&app_item);
    let app_menu = NSMenu::new(mtm);
    let quit = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str("Quit"),
            Some(sel!(quit:)),
            &NSString::from_str("q"),
        )
    };
    unsafe { quit.setTarget(Some(&menu_target)) };
    app_menu.addItem(&quit);
    app_item.setSubmenu(Some(&app_menu));
    app.setMainMenu(Some(&main_menu));
    // Force the thread-local ring to allocate now, not on the hot path.
    PENDING.with(|_| {});
    Ok(EventLoop::with_backend(Box::new(MacEventLoop {
        exit: Arc::new(AtomicBool::new(false)),
        mtm,
        _menu_target: menu_target,
    })))
}

impl MacEventLoop {
    /// Pulls AppKit events, translates them, and forwards each to the application.
    /// If `first_wait` is set, blocks up to that long for the first event.
    fn pump(&self, first_wait: Option<Duration>) {
        let app = NSApplication::sharedApplication(self.mtm);
        let mode = unsafe { NSDefaultRunLoopMode };
        let first_expiry =
            first_wait.map(|d| NSDate::dateWithTimeIntervalSinceNow(d.as_secs_f64()));
        let mut expiry = first_expiry;
        let past = NSDate::distantPast();
        loop {
            let deadline = expiry.as_deref().unwrap_or(&past);
            let Some(event) = app.nextEventMatchingMask_untilDate_inMode_dequeue(
                NSEventMask(u64::MAX),
                Some(deadline),
                mode,
                true,
            ) else {
                break;
            };
            expiry = None;
            self.dispatch(&event);
            app.sendEvent(&event);
        }
        reap_windows(self.mtm);
    }

    /// Routes one AppKit event to the window it belongs to.
    fn dispatch(&self, event: &NSEvent) {
        let Some(target) = event.window(self.mtm) else {
            return;
        };
        let target_ptr: *const NSWindow = &*target;
        WINDOWS.with(|w| {
            if let Some(record) = w
                .borrow()
                .iter()
                .find(|r| std::ptr::eq(&*r.ns_window, target_ptr))
            {
                translate(record, event, self.mtm);
            }
        });
    }

    /// Applies window commands and state diffs, then moves queued events to `sink`.
    fn deliver(&self, sink: &mut EventSink<'_>) -> PollStatus {
        WINDOWS.with(|w| {
            let windows = w.borrow();
            for record in windows.iter() {
                sync_record(record, self.mtm);
            }
        });
        reap_windows(self.mtm);

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

impl EventLoopBackend for MacEventLoop {
    fn poll(&mut self, sink: &mut EventSink<'_>) -> PollStatus {
        self.pump(None);
        self.deliver(sink)
    }

    fn wait(&mut self, sink: &mut EventSink<'_>, timeout: Duration) -> PollStatus {
        let have_pending = PENDING.with(|p| !p.borrow().is_empty());
        self.pump(if have_pending { None } else { Some(timeout) });
        self.deliver(sink)
    }

    fn request_exit(&self) {
        self.exit.store(true, Ordering::Release);
    }

    fn monitors(&self) -> Vec<tpt_window_core::MonitorInfo> {
        let Some(mtm) = MainThreadMarker::new() else {
            return Vec::new();
        };
        let main_height = screen_height(mtm);
        // AppKit lists the primary display first.
        NSScreen::screens(mtm)
            .iter()
            .enumerate()
            .map(|(i, screen)| screen_info(&screen, main_height, i == 0))
            .collect()
    }

    fn create_window(
        &self,
        attrs: &WindowAttributes,
    ) -> Result<Arc<dyn WindowBackend>, WindowError> {
        let mtm = self.mtm;
        let mut style = if attrs.decorations {
            NSWindowStyleMask::Titled
                | NSWindowStyleMask::Closable
                | NSWindowStyleMask::Miniaturizable
        } else {
            NSWindowStyleMask::Borderless
        };
        if attrs.resizable && attrs.decorations {
            style |= NSWindowStyleMask::Resizable;
        }

        let size = attrs.inner_size.unwrap_or(LogicalSize::new(800.0, 600.0));
        let rect = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(size.width, size.height));
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                rect,
                style,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        // Records own the window; AppKit must not release it behind our back.
        unsafe { window.setReleasedWhenClosed(false) };
        window.setTitle(&NSString::from_str(&attrs.title));
        if let Some(min) = attrs.min_size {
            window.setContentMinSize(NSSize::new(min.width, min.height));
        }
        if let Some(max) = attrs.max_size {
            window.setMaxSize(NSSize::new(max.width, max.height));
        }
        match attrs.position {
            Some(p) => {
                window.setFrameTopLeftPoint(NSPoint::new(p.x, screen_height(mtm) - p.y));
            }
            None => window.center(),
        }

        let id = WindowId(NEXT_ID.fetch_add(1, Ordering::Relaxed));
        // Replace the default content view with one that reports paint requests. It takes the
        // old view's frame, so the layout is unchanged.
        let frame = window
            .contentView()
            .ok_or_else(|| WindowError::Os("window has no content view".into()))?
            .frame();
        let content = ContentView::new(mtm, id, frame);
        window.setContentView(Some(&content));
        let view = window
            .contentView()
            .ok_or_else(|| WindowError::Os("window has no content view".into()))?;
        let view_ptr = Retained::as_ptr(&view) as usize;

        // Enter/exit: the tracking area covers the whole view and follows its size.
        let tracking = unsafe {
            NSTrackingArea::initWithRect_options_owner_userInfo(
                NSTrackingArea::alloc(),
                view.bounds(),
                NSTrackingAreaOptions::MouseEnteredAndExited
                    | NSTrackingAreaOptions::ActiveAlways
                    | NSTrackingAreaOptions::InVisibleRect,
                Some(&view),
                None,
            )
        };
        view.addTrackingArea(&tracking);
        // Without this, AppKit sends no mouse-moved events unless a button is held, so
        // `CursorMoved` would be missing for normal pointer movement.
        window.setAcceptsMouseMovedEvents(true);

        let delegate = WindowDelegate::new(mtm, id);
        window.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        let data = Arc::new(WindowData {
            id,
            state: SharedWindowState::new(WindowState {
                size: physical_size(&window),
                position: physical_position(&window, mtm),
                scale_factor: window.backingScaleFactor(),
                focused: false,
                visible: attrs.visible,
            }),
            commands: Mutex::new(VecDeque::new()),
            view: AtomicUsize::new(view_ptr),
            destroy_requested: AtomicBool::new(false),
        });

        if attrs.visible {
            let app = NSApplication::sharedApplication(mtm);
            #[allow(deprecated)]
            app.activateIgnoringOtherApps(true);
            window.makeKeyAndOrderFront(None);
        }

        WINDOWS.with(|w| {
            w.borrow_mut().push(Record {
                ns_window: window,
                _delegate: delegate,
                data: data.clone(),
            });
        });
        Ok(Arc::new(MacWindow { data }))
    }
}

/// A macOS window handle. Dropping the last handle destroys the window on the next
/// poll.
pub struct MacWindow {
    data: Arc<WindowData>,
}

impl WindowBackend for MacWindow {
    fn id(&self) -> WindowId {
        self.data.id
    }

    fn state(&self) -> &Arc<SharedWindowState> {
        &self.data.state
    }

    fn raw_window_handle(&self) -> Result<RawWindowHandle, WindowError> {
        let ptr = NonNull::new(self.data.view.load(Ordering::Acquire) as *mut c_void)
            .ok_or(WindowError::Destroyed)?;
        Ok(RawWindowHandle::AppKit(AppKitWindowHandle::new(ptr)))
    }

    fn raw_display_handle(&self) -> Result<RawDisplayHandle, WindowError> {
        Ok(RawDisplayHandle::AppKit(AppKitDisplayHandle::new()))
    }

    fn current_monitor(&self) -> Option<tpt_window_core::MonitorInfo> {
        let mtm = MainThreadMarker::new()?;
        let addr = self.data.view.load(Ordering::Acquire);
        if addr == 0 {
            return None;
        }
        // SAFETY: the address is this window's content view. The window keeps it alive until
        // destroy, which zeroes the address first, so it is valid for this borrow.
        let view: &NSView = unsafe { &*(addr as *const NSView) };
        let screen = view.window()?.screen()?;
        let screens = NSScreen::screens(mtm);
        let primary = screens
            .iter()
            .next()
            .is_some_and(|first| std::ptr::eq(&*first, &*screen));
        Some(screen_info(&screen, screen_height(mtm), primary))
    }

    fn send_command(&self, command: WindowCommand) {
        self.data
            .commands
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(command);
    }
}

impl Drop for MacWindow {
    fn drop(&mut self) {
        // AppKit objects are main-thread only, so the poll loop does the teardown.
        self.data.destroy_requested.store(true, Ordering::Release);
    }
}
