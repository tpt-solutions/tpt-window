// SPDX-License-Identifier: MIT OR Apache-2.0
//! tpt-window: engine-first windowing, input and surface management.
//!
//! This facade re-exports [`tpt_window_core`] and picks the platform backend.
//!
//! ```no_run
//! use tpt_window::*;
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
#![warn(missing_docs)]

pub use tpt_window_core::*;

/// Creates the event loop for the current platform.
///
/// The spec writes this as `EventLoop::new()`; an inherent constructor cannot live in
/// the facade because `EventLoop` is defined in `tpt-window-core`.
///
/// Platforms without a backend yet (Linux without Wayland, X11 not written yet; see
/// todo.md Phase 3) return
/// [`WindowError::Unsupported`].
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
        tpt_window_sys_wayland::new_event_loop()
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
