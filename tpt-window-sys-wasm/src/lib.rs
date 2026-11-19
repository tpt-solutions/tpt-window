// SPDX-License-Identifier: MIT OR Apache-2.0
//! Web (WASM) backend for tpt-window, using `web-sys` and a `<canvas>` per window.
//!
//! * DOM listeners push into a pre-allocated ring; `poll` drains it into the caller's
//!   sink. A browser cannot block, so `wait` behaves like `poll`.
//! * Each window owns one canvas appended to `<body>`. The canvas backing store is sized
//!   in physical pixels (`css size * devicePixelRatio`).
//! * Listener closures are intentionally leaked (`forget`) for the page lifetime.
//! * `WindowBackend` requires `Send + Sync`; wasm32-unknown-unknown is single-threaded
//!   (no `atomics` target feature), which is what makes the `unsafe impl`s below sound.
//!
//! **Status:** compile-checked only; not yet exercised in a browser (see todo.md).
#![cfg(target_arch = "wasm32")]
#![warn(missing_docs)]

use std::cell::RefCell;
use std::ffi::c_void;
use std::ptr::NonNull;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use raw_window_handle::{
    RawDisplayHandle, RawWindowHandle, WebCanvasWindowHandle, WebDisplayHandle,
};
use tpt_window_core::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use web_sys::{
    Document, Event as DomEvent, EventTarget, FocusEvent, HtmlCanvasElement, KeyboardEvent,
    MouseEvent, TouchEvent, WheelEvent,
};

type Pending = Rc<RefCell<RingBuffer<Event>>>;

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Creates an [`EventLoop`] backed by the browser.
pub fn new_event_loop() -> Result<EventLoop, WindowError> {
    let window = web_sys::window().ok_or(WindowError::Unsupported("no global window"))?;
    window
        .document()
        .ok_or(WindowError::Unsupported("no document"))?;
    Ok(EventLoop::with_backend(Box::new(WebEventLoop {
        // Shared with every window through the same thread-local ring.
        pending: PENDING.with(|p| p.clone()),
        exit: Arc::new(AtomicBool::new(false)),
    })))
}

struct WebEventLoop {
    pending: Pending,
    exit: Arc<AtomicBool>,
}

impl WebEventLoop {
    fn deliver(&self, sink: &mut EventSink<'_>) -> PollStatus {
        let mut delivered = false;
        loop {
            let Some(ev) = self.pending.borrow().front() else {
                break;
            };
            match sink.push(ev) {
                EventStatus::Accepted => {
                    self.pending.borrow_mut().pop();
                    delivered = true;
                }
                _ => break,
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

impl EventLoopBackend for WebEventLoop {
    fn poll(&mut self, sink: &mut EventSink<'_>) -> PollStatus {
        self.deliver(sink)
    }

    fn wait(&mut self, sink: &mut EventSink<'_>, _timeout: Duration) -> PollStatus {
        // Browsers cannot block the main thread; behave like poll.
        self.deliver(sink)
    }

    fn request_exit(&self) {
        self.exit.store(true, Ordering::Release);
    }

    fn create_window(
        &self,
        attrs: &WindowAttributes,
    ) -> Result<Arc<dyn WindowBackend>, WindowError> {
        let win = web_sys::window().ok_or(WindowError::Unsupported("no global window"))?;
        let doc: Document = win
            .document()
            .ok_or(WindowError::Unsupported("no document"))?;
        let canvas: HtmlCanvasElement = doc
            .create_element("canvas")
            .map_err(js_err)?
            .dyn_into()
            .map_err(|_| WindowError::Os("not a canvas".into()))?;
        canvas.set_tab_index(0); // focusable, so it receives key events
        let style = canvas.style();
        let logical = attrs.inner_size.unwrap_or(LogicalSize::new(800.0, 600.0));
        style
            .set_property("width", &format!("{}px", logical.width))
            .map_err(js_err)?;
        style
            .set_property("height", &format!("{}px", logical.height))
            .map_err(js_err)?;
        if !attrs.visible {
            style.set_property("display", "none").map_err(js_err)?;
        }
        doc.body()
            .ok_or(WindowError::Unsupported("no body"))?
            .append_child(&canvas)
            .map_err(js_err)?;
        doc.set_title(&attrs.title);

        let dpr = win.device_pixel_ratio();
        let size = logical.to_physical(dpr);
        canvas.set_width(size.width);
        canvas.set_height(size.height);

        let id = WindowId(NEXT_ID.fetch_add(1, Ordering::Relaxed));
        let state = SharedWindowState::new(WindowState {
            size,
            position: PhysicalPosition::new(0, 0),
            scale_factor: dpr,
            focused: false,
            visible: attrs.visible,
        });
        let pending = PENDING.with(|p| p.clone());
        let ctx = Rc::new(Ctx {
            id,
            state: state.clone(),
            pending,
            canvas: canvas.clone(),
        });
        install_listeners(&ctx);
        ctx.sync_size();
        Ok(Arc::new(WebWindow {
            ctx,
            handle: Box::new(JsValue::from(canvas)),
        }))
    }
}

thread_local! {
    // One ring per page, shared by the event loop and all windows.
    static PENDING: Pending = Rc::new(RefCell::new(
        RingBuffer::with_capacity(4096).with_policy(OverflowPolicy::DropOldest),
    ));
}

fn js_err(e: JsValue) -> WindowError {
    WindowError::Os(format!("{e:?}"))
}

struct Ctx {
    id: WindowId,
    state: Arc<SharedWindowState>,
    pending: Pending,
    canvas: HtmlCanvasElement,
}

impl Ctx {
    fn push(&self, event: WindowEvent) {
        self.pending.borrow_mut().push(Event {
            window: self.id,
            event,
        });
    }

    fn dpr() -> f64 {
        web_sys::window().map_or(1.0, |w| w.device_pixel_ratio())
    }

    /// Re-reads the CSS size and DPR, resizes the backing store, emits events.
    fn sync_size(&self) {
        let dpr = Self::dpr();
        let w = (f64::from(self.canvas.client_width()) * dpr)
            .round()
            .max(0.0) as u32;
        let h = (f64::from(self.canvas.client_height()) * dpr)
            .round()
            .max(0.0) as u32;
        let size = PhysicalSize::new(w, h);
        let old = self.state.snapshot();
        if w > 0 && h > 0 && (old.size != size || self.canvas.width() != w) {
            self.canvas.set_width(w);
            self.canvas.set_height(h);
        }
        let scale_changed = (old.scale_factor - dpr).abs() > f64::EPSILON;
        self.state.update(|s| {
            s.size = size;
            s.scale_factor = dpr;
        });
        if scale_changed {
            self.push(WindowEvent::ScaleFactorChanged {
                scale_factor: dpr,
                new_inner_size: size,
            });
        }
        if old.size != size {
            self.push(WindowEvent::Resized(size));
        }
    }

    fn mouse_pos(&self, e: &MouseEvent) -> PhysicalPosition<f64> {
        let dpr = Self::dpr();
        PhysicalPosition::new(f64::from(e.offset_x()) * dpr, f64::from(e.offset_y()) * dpr)
    }
}

fn listen<E: JsCast + 'static>(target: &EventTarget, name: &str, mut f: impl FnMut(E) + 'static) {
    let closure =
        Closure::<dyn FnMut(DomEvent)>::new(move |e: DomEvent| f(e.unchecked_into::<E>()));
    let _ = target.add_event_listener_with_callback(name, closure.as_ref().unchecked_ref());
    closure.forget();
}

fn button(b: i16) -> MouseButton {
    match b {
        0 => MouseButton::Left,
        1 => MouseButton::Middle,
        2 => MouseButton::Right,
        3 => MouseButton::Back,
        4 => MouseButton::Forward,
        n => MouseButton::Other(n.max(0) as u16),
    }
}

fn install_listeners(ctx: &Rc<Ctx>) {
    let t: &EventTarget = ctx.canvas.as_ref();

    let c = ctx.clone();
    listen(t, "mousemove", move |e: MouseEvent| {
        c.push(WindowEvent::CursorMoved {
            position: c.mouse_pos(&e),
            pressure: 0.0,
        });
    });
    let c = ctx.clone();
    listen(t, "mouseenter", move |_: DomEvent| {
        c.push(WindowEvent::CursorEntered)
    });
    let c = ctx.clone();
    listen(t, "mouseleave", move |_: DomEvent| {
        c.push(WindowEvent::CursorLeft)
    });
    for (name, state) in [
        ("mousedown", ElementState::Pressed),
        ("mouseup", ElementState::Released),
    ] {
        let c = ctx.clone();
        listen(t, name, move |e: MouseEvent| {
            if state == ElementState::Pressed {
                let _ = c.canvas.focus();
            }
            c.push(WindowEvent::MouseInput {
                state,
                button: button(e.button()),
            });
        });
    }
    let c = ctx.clone();
    listen(t, "contextmenu", move |e: DomEvent| {
        e.prevent_default();
        let _ = &c;
    });
    let c = ctx.clone();
    listen(t, "wheel", move |e: WheelEvent| {
        e.prevent_default();
        // Web y grows downwards; flip so positive means "scroll up", matching Win32.
        let delta = if e.delta_mode() == 0 {
            MouseScrollDelta::PixelDelta {
                x: e.delta_x(),
                y: -e.delta_y(),
            }
        } else {
            MouseScrollDelta::LineDelta {
                x: e.delta_x() as f32,
                y: -e.delta_y() as f32,
            }
        };
        c.push(WindowEvent::MouseWheel {
            delta,
            phase: TouchPhase::Moved,
        });
    });

    for (name, state) in [
        ("keydown", ElementState::Pressed),
        ("keyup", ElementState::Released),
    ] {
        let c = ctx.clone();
        listen(t, name, move |e: KeyboardEvent| {
            let key_code = key_code(&e);
            c.push(WindowEvent::KeyboardInput {
                state,
                key_code,
                physical_key: PhysicalKey::Code(code_to_key(&e.code())),
            });
            // Printable single characters become text, unless a shortcut modifier is held.
            if state == ElementState::Pressed && !e.ctrl_key() && !e.meta_key() {
                let k = e.key();
                if k.chars().count() == 1 {
                    c.push(WindowEvent::Ime(ImeEvent::Commit {
                        text: ImeText::new(&k),
                        continued: false,
                    }));
                }
            }
        });
    }

    for name in ["focus", "blur"] {
        let c = ctx.clone();
        let focused = name == "focus";
        listen(t, name, move |_: FocusEvent| {
            c.state.update(|s| s.focused = focused);
            c.push(WindowEvent::Focused(focused));
        });
    }

    for (name, phase) in [
        ("touchstart", TouchPhase::Started),
        ("touchmove", TouchPhase::Moved),
        ("touchend", TouchPhase::Ended),
        ("touchcancel", TouchPhase::Cancelled),
    ] {
        let c = ctx.clone();
        listen(t, name, move |e: TouchEvent| {
            e.prevent_default();
            let rect = c.canvas.get_bounding_client_rect();
            let dpr = Ctx::dpr();
            let list = e.changed_touches();
            for i in 0..list.length() {
                if let Some(touch) = list.get(i) {
                    c.push(WindowEvent::Touch(tpt_window_core::TouchEvent {
                        id: touch.identifier() as u64,
                        phase,
                        position: PhysicalPosition::new(
                            (f64::from(touch.client_x()) - rect.left()) * dpr,
                            (f64::from(touch.client_y()) - rect.top()) * dpr,
                        ),
                        force: touch.force(),
                    }));
                }
            }
        });
    }

    // Resize via ResizeObserver (catches container-driven changes too).
    let c = ctx.clone();
    let cb = Closure::<dyn FnMut(js_sys::Array)>::new(move |_entries: js_sys::Array| c.sync_size());
    if let Ok(observer) = web_sys::ResizeObserver::new(cb.as_ref().unchecked_ref()) {
        observer.observe(&ctx.canvas);
    }
    cb.forget();
}

/// Layout-translated key: single letters/digits from `KeyboardEvent.key`, otherwise by code.
fn key_code(e: &KeyboardEvent) -> KeyCode {
    let k = e.key();
    let mut chars = k.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c.is_ascii_alphabetic() {
            return code_to_key(&format!("Key{}", c.to_ascii_uppercase()));
        }
        if c.is_ascii_digit() {
            return code_to_key(&format!("Digit{c}"));
        }
    }
    code_to_key(&e.code())
}

fn code_to_key(code: &str) -> KeyCode {
    use KeyCode::*;
    if let Some(l) = code.strip_prefix("Key") {
        if let Some(c) = l.chars().next().filter(|c| c.is_ascii_uppercase()) {
            return LETTERS[(c as u8 - b'A') as usize];
        }
    }
    if let Some(d) = code.strip_prefix("Digit") {
        if let Some(c) = d.chars().next().filter(char::is_ascii_digit) {
            return DIGITS[(c as u8 - b'0') as usize];
        }
    }
    if let Some(n) = code.strip_prefix("Numpad") {
        if let Some(c) = n.chars().next().filter(char::is_ascii_digit) {
            return NUMPAD[(c as u8 - b'0') as usize];
        }
    }
    if let Some(n) = code.strip_prefix('F') {
        if let Ok(i @ 1..=12) = n.parse::<usize>() {
            return FKEYS[i - 1];
        }
    }
    match code {
        "ArrowUp" => ArrowUp,
        "ArrowDown" => ArrowDown,
        "ArrowLeft" => ArrowLeft,
        "ArrowRight" => ArrowRight,
        "Escape" => Escape,
        "Enter" => Enter,
        "Space" => Space,
        "Tab" => Tab,
        "Backspace" => Backspace,
        "Delete" => Delete,
        "Insert" => Insert,
        "Home" => Home,
        "End" => End,
        "PageUp" => PageUp,
        "PageDown" => PageDown,
        "ShiftLeft" => ShiftLeft,
        "ShiftRight" => ShiftRight,
        "ControlLeft" => ControlLeft,
        "ControlRight" => ControlRight,
        "AltLeft" => AltLeft,
        "AltRight" => AltRight,
        "MetaLeft" => SuperLeft,
        "MetaRight" => SuperRight,
        "CapsLock" => CapsLock,
        "Minus" => Minus,
        "Equal" => Equal,
        "BracketLeft" => BracketLeft,
        "BracketRight" => BracketRight,
        "Backslash" => Backslash,
        "Semicolon" => Semicolon,
        "Quote" => Quote,
        "Backquote" => Backquote,
        "Comma" => Comma,
        "Period" => Period,
        "Slash" => Slash,
        "NumpadAdd" => NumpadAdd,
        "NumpadSubtract" => NumpadSubtract,
        "NumpadMultiply" => NumpadMultiply,
        "NumpadDivide" => NumpadDivide,
        "NumpadEnter" => NumpadEnter,
        "NumpadDecimal" => NumpadDecimal,
        _ => Unidentified,
    }
}

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
const NUMPAD: [KeyCode; 10] = {
    use KeyCode::*;
    [
        Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    ]
};
const FKEYS: [KeyCode; 12] = {
    use KeyCode::*;
    [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12]
};

/// A canvas-backed window.
pub struct WebWindow {
    ctx: Rc<Ctx>,
    /// Heap-pinned `JsValue` whose address is handed out as the canvas handle.
    handle: Box<JsValue>,
}

// SAFETY: wasm32-unknown-unknown without the `atomics` feature has exactly one thread,
// so no value is ever accessed concurrently.
unsafe impl Send for WebWindow {}
unsafe impl Sync for WebWindow {}

impl WindowBackend for WebWindow {
    fn id(&self) -> WindowId {
        self.ctx.id
    }

    fn state(&self) -> &Arc<SharedWindowState> {
        &self.ctx.state
    }

    fn raw_window_handle(&self) -> Result<RawWindowHandle, WindowError> {
        let ptr = NonNull::from(&*self.handle).cast::<c_void>();
        Ok(RawWindowHandle::WebCanvas(WebCanvasWindowHandle::new(ptr)))
    }

    fn raw_display_handle(&self) -> Result<RawDisplayHandle, WindowError> {
        Ok(RawDisplayHandle::Web(WebDisplayHandle::new()))
    }

    fn send_command(&self, command: WindowCommand) {
        let ctx = &self.ctx;
        match command {
            WindowCommand::SetTitle(t) => {
                if let Some(d) = web_sys::window().and_then(|w| w.document()) {
                    d.set_title(&t);
                }
            }
            WindowCommand::SetSize(size) => {
                let dpr = Ctx::dpr();
                let style = ctx.canvas.style();
                let _ = style.set_property("width", &format!("{}px", f64::from(size.width) / dpr));
                let _ =
                    style.set_property("height", &format!("{}px", f64::from(size.height) / dpr));
                ctx.sync_size();
            }
            WindowCommand::SetVisible(v) => {
                let _ = ctx
                    .canvas
                    .style()
                    .set_property("display", if v { "" } else { "none" });
                ctx.state.update(|s| s.visible = v);
            }
            WindowCommand::RequestRedraw => {
                let c = ctx.clone();
                let cb = Closure::once_into_js(move || c.push(WindowEvent::RedrawRequested));
                if let Some(w) = web_sys::window() {
                    let _ = w.request_animation_frame(cb.unchecked_ref());
                }
            }
            // A page cannot move its canvas relative to the screen.
            WindowCommand::SetPosition(_) => {}
            _ => {}
        }
    }
}
