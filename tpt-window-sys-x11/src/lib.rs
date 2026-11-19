// SPDX-License-Identifier: MIT OR Apache-2.0
//! X11 backend for tpt-window, built on Xlib loaded at runtime (`x11-dl`).
//!
//! Xlib rather than xcb: wgpu's Xlib surface needs the `Display*`, and xcb's raw
//! connection is not exposed by the pure-Rust `x11rb` client.
//!
//! Design notes:
//! - All Xlib calls happen on the thread that created the [`EventLoop`]. Handles
//!   on other threads only queue commands, applied on the next poll.
//! - Scale factor is 1.0. Xft DPI scaling is not read yet.
//! - Auto-repeat arrives as release/press pairs unless detectable auto-repeat is
//!   enabled. Key repeat is not generated.
//! - Window closing is requested through `WM_DELETE_WINDOW`, so the application can
//!   veto it. A close the application accepts destroys the window.
//! - X11 errors are ignored by a no-op error handler instead of exiting the process.
//!
//! Only Linux builds include this crate's code.
#![cfg(target_os = "linux")]
#![warn(missing_docs)]

use std::cell::RefCell;
use std::collections::VecDeque;
use std::ffi::{CString, c_char, c_int, c_long, c_ulong, c_void};
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raw_window_handle::{RawDisplayHandle, RawWindowHandle, XlibDisplayHandle, XlibWindowHandle};
use tpt_window_core::*;
use x11_dl::xlib::{self, Display, XErrorEvent, XEvent, XKeyEvent, XSizeHints, Xlib};

static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Ignores X protocol errors. Xlib's default handler exits the process.
unsafe extern "C" fn ignore_x_error(_display: *mut Display, _event: *mut XErrorEvent) -> c_int {
    0
}

// ---------------------------------------------------------------------------
// Per-window data
// ---------------------------------------------------------------------------

/// State shared between the event-loop record and the `Window` handle.
struct WindowData {
    id: WindowId,
    state: Arc<SharedWindowState>,
    commands: Mutex<VecDeque<WindowCommand>>,
    /// The X window id, or 0 once destroyed.
    window: AtomicUsize,
    /// `Display*` and screen, for raw handles.
    display: usize,
    screen: c_int,
    destroy_requested: AtomicBool,
}

/// Event-loop side of a window.
struct Record {
    window: c_ulong,
    data: Arc<WindowData>,
    size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    focused: bool,
}

// ---------------------------------------------------------------------------
// Backend
// ---------------------------------------------------------------------------

/// The X11 event loop.
pub struct X11EventLoop {
    xlib: Xlib,
    display: *mut Display,
    screen: c_int,
    wm_delete: c_ulong,
    records: RefCell<Vec<Record>>,
    pending: RefCell<VecDeque<Event>>,
    exit: Arc<AtomicBool>,
}

/// Creates an [`EventLoop`] connected to `$DISPLAY`. Fails when Xlib is not
/// installed or no X server is reachable.
pub fn new_event_loop() -> Result<EventLoop, WindowError> {
    let xlib = Xlib::open().map_err(|e| WindowError::Os(format!("cannot load Xlib: {e}")))?;
    // SAFETY: plain Xlib calls on a freshly opened display, with the error handler
    // installed before any other request is sent.
    let display = unsafe {
        (xlib.XSetErrorHandler)(Some(ignore_x_error));
        (xlib.XOpenDisplay)(ptr::null())
    };
    if display.is_null() {
        return Err(WindowError::Os(
            "cannot open the X display (is DISPLAY set?)".into(),
        ));
    }
    // SAFETY: `display` is valid until XCloseDisplay in Drop.
    let (screen, wm_delete) = unsafe {
        let screen = (xlib.XDefaultScreen)(display);
        let atom = (xlib.XInternAtom)(display, c"WM_DELETE_WINDOW".as_ptr(), xlib::False);
        (screen, atom)
    };
    // Force the thread-local ring to allocate now, not on the hot path.
    Ok(EventLoop::with_backend(Box::new(X11EventLoop {
        xlib,
        display,
        screen,
        wm_delete,
        records: RefCell::new(Vec::new()),
        pending: RefCell::new(VecDeque::with_capacity(4096)),
        exit: Arc::new(AtomicBool::new(false)),
    })))
}

fn push_text(pending: &mut VecDeque<Event>, id: WindowId, text: &str) {
    let mut rest: &str = text;
    while !rest.is_empty() {
        let (chunk, next) = ImeText::take_chunk(rest);
        let continued = !next.is_empty();
        pending.push_back(Event {
            window: id,
            event: WindowEvent::Ime(ImeEvent::Commit {
                text: chunk,
                continued,
            }),
        });
        rest = next;
    }
}

fn mouse_button(code: u32) -> Option<MouseButton> {
    match code {
        1 => Some(MouseButton::Left),
        2 => Some(MouseButton::Middle),
        3 => Some(MouseButton::Right),
        8 => Some(MouseButton::Back),
        9 => Some(MouseButton::Forward),
        _ => None,
    }
}

impl X11EventLoop {
    fn record_index(&self, window: c_ulong) -> Option<usize> {
        self.records
            .borrow()
            .iter()
            .position(|r| r.window == window)
    }

    /// Reads every queued X event and turns it into tpt-window events.
    fn drain_x_events(&mut self) {
        // SAFETY: `display` is open; XPending only reads the queue.
        while unsafe { (self.xlib.XPending)(self.display) } > 0 {
            let mut ev = XEvent { type_: 0 };
            // SAFETY: XNextEvent fills `ev`; reading the union member is valid for the
            // event type checked below.
            unsafe { (self.xlib.XNextEvent)(self.display, &mut ev) };
            self.handle_event(&mut ev);
        }
    }

    fn handle_event(&mut self, ev: &mut XEvent) {
        // SAFETY: `ev.type_` tells which union member is initialised.
        let kind = unsafe { ev.type_ };
        match kind {
            xlib::ConfigureNotify => {
                // SAFETY: ConfigureNotify fills `configure`.
                let c = unsafe { ev.configure };
                let Some(i) = self.record_index(c.window) else {
                    return;
                };
                let size = PhysicalSize::new(c.width.max(1) as u32, c.height.max(1) as u32);
                let position = PhysicalPosition::new(c.x, c.y);
                let rec = &mut self.records.borrow_mut()[i];
                let id = rec.data.id;
                let mut out = self.pending.borrow_mut();
                if size != rec.size {
                    rec.size = size;
                    rec.data.state.update(|s| s.size = size);
                    out.push_back(Event {
                        window: id,
                        event: WindowEvent::Resized(size),
                    });
                }
                if position != rec.position {
                    rec.position = position;
                    rec.data.state.update(|s| s.position = position);
                    out.push_back(Event {
                        window: id,
                        event: WindowEvent::Moved(position),
                    });
                }
            }
            xlib::KeyPress | xlib::KeyRelease => {
                // SAFETY: key events fill `key`.
                let mut key = unsafe { ev.key };
                let Some(i) = self.record_index(key.window) else {
                    return;
                };
                let id = self.records.borrow()[i].data.id;
                // X keycodes are evdev codes plus 8 on Linux.
                let code = key_from_evdev(key.keycode.saturating_sub(8));
                let state = if kind == xlib::KeyPress {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                };
                let mut out = self.pending.borrow_mut();
                out.push_back(Event {
                    window: id,
                    event: WindowEvent::KeyboardInput {
                        state,
                        key_code: code,
                        physical_key: PhysicalKey::Code(code),
                    },
                });
                if kind == xlib::KeyPress {
                    let text = self.lookup_text(&mut key);
                    if !text.is_empty() && text.chars().all(|c| !c.is_control()) {
                        push_text(&mut out, id, &text);
                    }
                }
            }
            xlib::ButtonPress | xlib::ButtonRelease => {
                // SAFETY: button events fill `button`.
                let b = unsafe { ev.button };
                let Some(i) = self.record_index(b.window) else {
                    return;
                };
                let id = self.records.borrow()[i].data.id;
                let pressed = kind == xlib::ButtonPress;
                let mut out = self.pending.borrow_mut();
                // Buttons 4-7 are the wheel; they only send presses.
                match b.button {
                    4..=7 if pressed => {
                        let (x, y) = match b.button {
                            4 => (0.0, 1.0),
                            5 => (0.0, -1.0),
                            6 => (-1.0, 0.0),
                            _ => (1.0, 0.0),
                        };
                        out.push_back(Event {
                            window: id,
                            event: WindowEvent::MouseWheel {
                                delta: MouseScrollDelta::LineDelta { x, y },
                                phase: TouchPhase::Moved,
                            },
                        });
                    }
                    4..=7 => {}
                    code => {
                        if let Some(button) = mouse_button(code) {
                            out.push_back(Event {
                                window: id,
                                event: WindowEvent::MouseInput {
                                    state: if pressed {
                                        ElementState::Pressed
                                    } else {
                                        ElementState::Released
                                    },
                                    button,
                                },
                            });
                        }
                    }
                }
            }
            xlib::MotionNotify => {
                // SAFETY: motion events fill `motion`.
                let m = unsafe { ev.motion };
                if let Some(i) = self.record_index(m.window) {
                    let id = self.records.borrow()[i].data.id;
                    self.pending.borrow_mut().push_back(Event {
                        window: id,
                        event: WindowEvent::CursorMoved {
                            position: PhysicalPosition::new(f64::from(m.x), f64::from(m.y)),
                            pressure: 0.0,
                        },
                    });
                }
            }
            xlib::EnterNotify | xlib::LeaveNotify => {
                // SAFETY: crossing events fill `crossing`.
                let c = unsafe { ev.crossing };
                if let Some(i) = self.record_index(c.window) {
                    let id = self.records.borrow()[i].data.id;
                    let event = if kind == xlib::EnterNotify {
                        WindowEvent::CursorEntered
                    } else {
                        WindowEvent::CursorLeft
                    };
                    self.pending
                        .borrow_mut()
                        .push_back(Event { window: id, event });
                }
            }
            xlib::FocusIn | xlib::FocusOut => {
                // SAFETY: focus events fill `focus_change`.
                let f = unsafe { ev.focus_change };
                if let Some(i) = self.record_index(f.window) {
                    let focused = kind == xlib::FocusIn;
                    let mut records = self.records.borrow_mut();
                    let rec = &mut records[i];
                    rec.focused = focused;
                    rec.data.state.update(|s| s.focused = focused);
                    self.pending.borrow_mut().push_back(Event {
                        window: rec.data.id,
                        event: WindowEvent::Focused(focused),
                    });
                }
            }
            xlib::ClientMessage => {
                // SAFETY: client messages fill `client_message`.
                let c = unsafe { ev.client_message };
                // SAFETY: `data.l` is the long-data view of a client message.
                let first = c.data.as_longs()[0];
                if first as c_ulong == self.wm_delete {
                    if let Some(i) = self.record_index(c.window) {
                        let id = self.records.borrow()[i].data.id;
                        self.pending.borrow_mut().push_back(Event {
                            window: id,
                            event: WindowEvent::CloseRequested,
                        });
                    }
                }
            }
            _ => {}
        }
    }

    /// Text for a key press, from the current keyboard mapping.
    fn lookup_text(&self, key: &mut XKeyEvent) -> String {
        let mut buf = [0_u8; 64];
        // SAFETY: `buf` is large enough for XLookupString's output; the keysym and
        // compose status are optional and passed as null.
        let n = unsafe {
            (self.xlib.XLookupString)(
                key,
                buf.as_mut_ptr() as *mut c_char,
                buf.len() as c_int,
                ptr::null_mut(),
                ptr::null_mut(),
            )
        };
        let n = n.clamp(0, buf.len() as c_int) as usize;
        String::from_utf8_lossy(&buf[..n]).into_owned()
    }

    /// Applies queued commands, destroys windows whose last handle dropped, and
    /// flushes the connection.
    fn sync_windows(&mut self) {
        let mut records = self.records.borrow_mut();
        let mut i = 0;
        while i < records.len() {
            let rec = &records[i];
            let cmds: Vec<WindowCommand> = {
                let mut q = rec.data.commands.lock().unwrap_or_else(|e| e.into_inner());
                q.drain(..).collect()
            };
            for cmd in cmds {
                match cmd {
                    WindowCommand::SetTitle(t) => {
                        let title = CString::new(t).unwrap_or_default();
                        // SAFETY: the window and display are live (not yet destroyed).
                        unsafe { (self.xlib.XStoreName)(self.display, rec.window, title.as_ptr()) };
                    }
                    WindowCommand::SetSize(s) => {
                        // SAFETY: as above.
                        unsafe {
                            (self.xlib.XResizeWindow)(
                                self.display,
                                rec.window,
                                s.width.max(1),
                                s.height.max(1),
                            )
                        };
                    }
                    WindowCommand::SetPosition(p) => {
                        // SAFETY: as above.
                        unsafe { (self.xlib.XMoveWindow)(self.display, rec.window, p.x, p.y) };
                    }
                    WindowCommand::SetVisible(v) => {
                        // SAFETY: as above.
                        unsafe {
                            if v {
                                (self.xlib.XMapWindow)(self.display, rec.window);
                            } else {
                                (self.xlib.XUnmapWindow)(self.display, rec.window);
                            }
                        };
                        rec.data.state.update(|s| s.visible = v);
                    }
                    // Xlib has no redraw request; the renderer presents on its own.
                    WindowCommand::RequestRedraw => {}
                    _ => {}
                }
            }
            if rec.data.destroy_requested.load(Ordering::Acquire) {
                let rec = records.remove(i);
                // SAFETY: the window is still live; it is destroyed exactly once here.
                unsafe { (self.xlib.XDestroyWindow)(self.display, rec.window) };
                rec.data.window.store(0, Ordering::Release);
                self.pending.borrow_mut().push_back(Event {
                    window: rec.data.id,
                    event: WindowEvent::Destroyed,
                });
            } else {
                i += 1;
            }
        }
        // SAFETY: the display is open.
        unsafe { (self.xlib.XFlush)(self.display) };
    }

    /// Moves queued events into the caller's sink.
    fn deliver(&self, sink: &mut EventSink<'_>) -> PollStatus {
        let mut pending = self.pending.borrow_mut();
        let mut delivered = false;
        while let Some(ev) = pending.front().copied() {
            match sink.push(ev) {
                EventStatus::Accepted => {
                    pending.pop_front();
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

    /// Waits up to `timeout` for the connection to become readable.
    fn wait_readable(&self, timeout: Duration) {
        // SAFETY: the display is open; the fd stays valid until XCloseDisplay.
        let fd = unsafe { (self.xlib.XConnectionNumber)(self.display) };
        let mut pfd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let ms = c_int::try_from(timeout.as_millis()).unwrap_or(c_int::MAX);
        // SAFETY: one valid pollfd.
        unsafe { libc::poll(&mut pfd, 1, ms) };
    }
}

impl EventLoopBackend for X11EventLoop {
    fn poll(&mut self, sink: &mut EventSink<'_>) -> PollStatus {
        self.drain_x_events();
        self.sync_windows();
        self.deliver(sink)
    }

    fn wait(&mut self, sink: &mut EventSink<'_>, timeout: Duration) -> PollStatus {
        let have_pending = !self.pending.borrow().is_empty();
        // SAFETY: the display is open.
        let queued = unsafe { (self.xlib.XPending)(self.display) } > 0;
        if !have_pending && !queued {
            self.wait_readable(timeout);
        }
        self.drain_x_events();
        self.sync_windows();
        self.deliver(sink)
    }

    fn request_exit(&self) {
        self.exit.store(true, Ordering::Release);
    }

    fn create_window(
        &self,
        attrs: &WindowAttributes,
    ) -> Result<Arc<dyn WindowBackend>, WindowError> {
        let inner = attrs.inner_size.unwrap_or(LogicalSize::new(800.0, 600.0));
        let size = PhysicalSize::new(inner.width.max(1.0) as u32, inner.height.max(1.0) as u32);
        let (x, y) = attrs
            .position
            .map_or((0, 0), |p| (p.x as c_int, p.y as c_int));
        let root = unsafe { (self.xlib.XRootWindow)(self.display, self.screen) };
        // SAFETY: the display is open; the window is configured below before use.
        let window = unsafe {
            (self.xlib.XCreateSimpleWindow)(
                self.display,
                root,
                x,
                y,
                size.width,
                size.height,
                0,
                0,
                0,
            )
        };
        if window == 0 {
            return Err(WindowError::Os("XCreateSimpleWindow failed".into()));
        }
        let mask: c_long = xlib::ExposureMask
            | xlib::KeyPressMask
            | xlib::KeyReleaseMask
            | xlib::ButtonPressMask
            | xlib::ButtonReleaseMask
            | xlib::PointerMotionMask
            | xlib::EnterWindowMask
            | xlib::LeaveWindowMask
            | xlib::StructureNotifyMask
            | xlib::FocusChangeMask;
        let title = CString::new(attrs.title.clone()).unwrap_or_default();
        let mut protocol = self.wm_delete;
        let mut hints: XSizeHints = unsafe { std::mem::zeroed() };
        hints.flags = 0;
        let fixed = !attrs.resizable || !attrs.decorations;
        if let Some(min) = attrs.min_size {
            hints.flags |= xlib::PMinSize;
            hints.min_width = min.width as c_int;
            hints.min_height = min.height as c_int;
        }
        if let Some(max) = attrs.max_size {
            hints.flags |= xlib::PMaxSize;
            hints.max_width = max.width as c_int;
            hints.max_height = max.height as c_int;
        }
        if fixed {
            hints.flags |= xlib::PMinSize | xlib::PMaxSize;
            hints.min_width = size.width as c_int;
            hints.min_height = size.height as c_int;
            hints.max_width = size.width as c_int;
            hints.max_height = size.height as c_int;
        }
        // SAFETY: all pointers passed refer to live locals or the title CString.
        unsafe {
            (self.xlib.XSelectInput)(self.display, window, mask);
            (self.xlib.XStoreName)(self.display, window, title.as_ptr());
            (self.xlib.XSetWMProtocols)(self.display, window, &mut protocol, 1);
            (self.xlib.XSetWMNormalHints)(self.display, window, &mut hints);
            if attrs.visible {
                (self.xlib.XMapWindow)(self.display, window);
            }
            (self.xlib.XFlush)(self.display);
        }

        let id = WindowId(NEXT_ID.fetch_add(1, Ordering::Relaxed));
        let data = Arc::new(WindowData {
            id,
            state: SharedWindowState::new(WindowState {
                size,
                position: PhysicalPosition::new(x, y),
                scale_factor: 1.0,
                focused: false,
                visible: attrs.visible,
            }),
            commands: Mutex::new(VecDeque::new()),
            window: AtomicUsize::new(window as usize),
            display: self.display as usize,
            screen: self.screen,
            destroy_requested: AtomicBool::new(false),
        });
        self.records.borrow_mut().push(Record {
            window,
            data: data.clone(),
            size,
            position: PhysicalPosition::new(x, y),
            focused: false,
        });
        Ok(Arc::new(X11Window { data }))
    }
}

impl Drop for X11EventLoop {
    fn drop(&mut self) {
        // Destroy what is left and close the connection. Handles that outlive the
        // loop see their window as destroyed.
        for rec in self.records.get_mut().drain(..) {
            rec.data.window.store(0, Ordering::Release);
            // SAFETY: the display is still open here.
            unsafe { (self.xlib.XDestroyWindow)(self.display, rec.window) };
        }
        // SAFETY: the display was opened in `new_event_loop` and is closed once.
        unsafe { (self.xlib.XCloseDisplay)(self.display) };
    }
}

/// An X11 window handle. Dropping the last handle destroys the window on the next
/// poll.
pub struct X11Window {
    data: Arc<WindowData>,
}

impl WindowBackend for X11Window {
    fn id(&self) -> WindowId {
        self.data.id
    }

    fn state(&self) -> &Arc<SharedWindowState> {
        &self.data.state
    }

    fn raw_window_handle(&self) -> Result<RawWindowHandle, WindowError> {
        let window = self.data.window.load(Ordering::Acquire);
        if window == 0 {
            return Err(WindowError::Destroyed);
        }
        Ok(RawWindowHandle::Xlib(XlibWindowHandle::new(
            window as c_ulong,
        )))
    }

    fn raw_display_handle(&self) -> Result<RawDisplayHandle, WindowError> {
        let display = NonNull::new(self.data.display as *mut c_void);
        Ok(RawDisplayHandle::Xlib(XlibDisplayHandle::new(
            display,
            self.data.screen,
        )))
    }

    fn send_command(&self, command: WindowCommand) {
        self.data
            .commands
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(command);
    }
}

impl Drop for X11Window {
    fn drop(&mut self) {
        self.data.destroy_requested.store(true, Ordering::Release);
    }
}
