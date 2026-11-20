// SPDX-License-Identifier: MIT OR Apache-2.0
//! tpt-window: engine-first windowing, input and surface management.
//!
//! This facade picks the platform backend and re-exports the public types of
//! [`tpt_window_core`]. You own the loop:
//!
//! ```no_run
//! use tpt_window::prelude::*;
//!
//! let mut event_loop = new_event_loop().unwrap();
//! let _window = WindowBuilder::new().with_title("hello").build(&event_loop).unwrap();
//! let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
//!
//! 'main: loop {
//!     // You own the loop: poll, drain, update, render.
//!     let status = event_loop.poll_events(&mut EventSink::new(&mut queue));
//!     for ev in queue.drain() {
//!         if ev.event == WindowEvent::CloseRequested { break 'main; }
//!     }
//!     if status == PollStatus::Exit { break; }
//! }
//! ```
//!
//! For a short program, [`run`] does the loop for you:
//!
//! ```no_run
//! use tpt_window::prelude::*;
//!
//! run(WindowBuilder::new().with_title("hello"), |_window, ev| {
//!     if ev.event == WindowEvent::CloseRequested { Flow::Exit } else { Flow::Continue }
//! })
//! .unwrap();
//! ```
//!
//! # Feature flags
//!
//! * `ecs`: `WindowResource` and `InputEventQueue` for engines with an ECS.
//! * `archon`: the capability bridge, as the `archon` module.
//! * `gamepad`: gamepad input, as the `gamepad` module. It is kept out of the window stream.
#![warn(missing_docs)]

use std::time::Duration;

pub use tpt_window_core::{
    CapabilityToken, DEFAULT_CAPACITY, DisplayScope, ElementState, Event, EventLoop,
    EventLoopBackend, EventQueue, EventSink, EventStatus, GamepadId, ImeEvent, ImeText, InlineText,
    InputScope, KeyCode, LogicalPosition, LogicalSize, ModifiersState, MouseButton,
    MouseScrollDelta, OverflowPolicy, PhysicalKey, PhysicalPosition, PhysicalSize, PollStatus,
    RawDisplayHandle, RawWindowHandle, RingBuffer, SharedWindowState, SurfaceHandle, TouchEvent,
    TouchPhase, Window, WindowAttributes, WindowBackend, WindowBuilder, WindowCommand, WindowError,
    WindowEvent, WindowId, WindowScope, WindowState,
};
#[cfg(feature = "ecs")]
pub use tpt_window_core::{InputEventQueue, WindowResource};

/// Sibling crates, behind their feature flags.
#[cfg(feature = "archon")]
pub use tpt_window_archon as archon;
#[cfg(feature = "gamepad")]
pub use tpt_window_gamepad as gamepad;

/// The names most programs need. `use tpt_window::prelude::*;` brings them all in.
pub mod prelude {
    pub use crate::{
        DEFAULT_CAPACITY, ElementState, Event, EventQueue, EventSink, Flow, KeyCode, LogicalSize,
        ModifiersState, MouseButton, PhysicalSize, PollStatus, Window, WindowBuilder, WindowError,
        WindowEvent, new_event_loop, run,
    };
}

/// A borrowed window that implements the `raw-window-handle` traits, so a renderer such as
/// wgpu can create a surface from it without `unsafe`:
///
/// ```ignore
/// let surface = instance.create_surface(SurfaceTarget::new(&window))?;
/// ```
///
/// The handles are valid while the window is alive, and the borrow keeps the window alive.
#[derive(Clone, Copy)]
pub struct SurfaceTarget<'a>(&'a Window);

impl<'a> SurfaceTarget<'a> {
    /// Borrows `window` as a surface target.
    pub fn new(window: &'a Window) -> Self {
        Self(window)
    }
}

impl raw_window_handle::HasWindowHandle for SurfaceTarget<'_> {
    fn window_handle(
        &self,
    ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError> {
        let raw = self
            .0
            .raw_window_handle()
            .map_err(|_| raw_window_handle::HandleError::Unavailable)?;
        // SAFETY: the raw handle is valid while the window exists. The returned borrow is tied
        // to `self`, which borrows the window, so the window outlives every use of the handle.
        Ok(unsafe { raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

impl raw_window_handle::HasDisplayHandle for SurfaceTarget<'_> {
    fn display_handle(
        &self,
    ) -> Result<raw_window_handle::DisplayHandle<'_>, raw_window_handle::HandleError> {
        let raw = self
            .0
            .raw_display_handle()
            .map_err(|_| raw_window_handle::HandleError::Unavailable)?;
        // SAFETY: as for `window_handle`. The display outlives the window it was taken from.
        Ok(unsafe { raw_window_handle::DisplayHandle::borrow_raw(raw) })
    }
}

/// What the handler given to [`run`] wants the loop to do next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flow {
    /// Keep running.
    Continue,
    /// Stop the loop after the events in this batch.
    Exit,
}

/// How long [`run`] waits for events when none are pending. It is an upper bound on latency
/// only when nothing happens; an event wakes the loop at once.
const RUN_WAIT: Duration = Duration::from_millis(100);

/// Runs one window built from `builder` until `handler` returns [`Flow::Exit`] or the
/// platform asks the application to stop.
///
/// The handler gets every event, including `YieldPoint`s, and may ignore what it does not need.
/// The window lives until this function returns. For anything beyond a simple program, write
/// the loop yourself: [`run`] hides the queue and the wait, and it has one window.
pub fn run<F>(builder: WindowBuilder, mut handler: F) -> Result<(), WindowError>
where
    F: FnMut(&Window, Event) -> Flow,
{
    let mut event_loop = new_event_loop()?;
    let window = builder.build(&event_loop)?;
    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    loop {
        let status = event_loop.step(&mut EventSink::new(&mut queue), RUN_WAIT);
        for ev in queue.drain() {
            if handler(&window, ev) == Flow::Exit {
                return Ok(());
            }
        }
        if status == PollStatus::Exit {
            return Ok(());
        }
    }
}

/// Creates the event loop for the current platform.
///
/// The spec writes this as `EventLoop::new()`; an inherent constructor cannot live in
/// the facade because `EventLoop` is defined in `tpt-window-core`.
///
/// Platforms without a backend yet return [`WindowError::Unsupported`]. On Linux, a
/// [`WindowError::NoDisplay`] or [`WindowError::Os`] error reports both the Wayland and the X11
/// attempt, so neither failure is hidden.
pub fn new_event_loop() -> Result<EventLoop, WindowError> {
    #[cfg(windows)]
    {
        tpt_window_sys_win32::new_event_loop()
    }
    #[cfg(target_os = "macos")]
    {
        tpt_window_sys_cocoa::new_event_loop()
    }
    #[cfg(target_os = "linux")]
    {
        // Prefer Wayland when it is running. If it fails, X11 is tried, and when both fail the
        // error names both.
        let wayland_error = if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            match tpt_window_sys_wayland::new_event_loop() {
                Ok(el) => return Ok(el),
                Err(e) => Some(e),
            }
        } else {
            None
        };
        match tpt_window_sys_x11::new_event_loop() {
            Ok(el) => Ok(el),
            Err(x11) => Err(match wayland_error {
                Some(wayland) => WindowError::NoDisplay(format!("Wayland: {wayland}; X11: {x11}")),
                None => x11,
            }),
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        tpt_window_sys_wasm::new_event_loop()
    }
    #[cfg(not(any(
        windows,
        target_os = "macos",
        target_os = "linux",
        target_arch = "wasm32"
    )))]
    {
        Err(WindowError::Unsupported("no backend for this platform yet"))
    }
}
