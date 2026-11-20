// SPDX-License-Identifier: MIT OR Apache-2.0
//! Traits implemented by platform backends, plus the shared window-state snapshot.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use crate::error::WindowError;
use crate::ids::WindowId;
use crate::security::{CapabilityToken, SurfaceHandle};
use crate::sink::EventSink;

/// Outcome of a poll or step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PollStatus {
    /// At least one OS event was delivered.
    Events,
    /// Nothing was pending.
    Idle,
    /// The loop was asked to exit; stop calling poll.
    Exit,
}

/// Cached window state. Getters read this snapshot; backends update it as events arrive.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowState {
    /// Inner size in physical pixels.
    pub size: PhysicalSize<u32>,
    /// Outer position in physical pixels.
    pub position: PhysicalPosition<i32>,
    /// DPI scale factor.
    pub scale_factor: f64,
    /// Has input focus.
    pub focused: bool,
    /// Is visible.
    pub visible: bool,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            size: PhysicalSize::new(800, 600),
            position: PhysicalPosition::new(0, 0),
            scale_factor: 1.0,
            focused: false,
            visible: true,
        }
    }
}

/// Thread-safe holder of a [`WindowState`] shared between a backend and its [`Window`](crate::Window)s.
#[derive(Debug, Default)]
pub struct SharedWindowState(Mutex<WindowState>);

impl SharedWindowState {
    /// Creates the holder.
    pub fn new(initial: WindowState) -> Arc<Self> {
        Arc::new(Self(Mutex::new(initial)))
    }

    /// Copies the current state (no syscalls, no allocation).
    pub fn snapshot(&self) -> WindowState {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Mutates the state (backend side).
    pub fn update(&self, f: impl FnOnce(&mut WindowState)) {
        f(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()));
    }
}

/// Mutations a [`Window`](crate::Window) asks the backend to apply on its next poll.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum WindowCommand {
    /// Change the title.
    SetTitle(String),
    /// Resize (physical pixels).
    SetSize(PhysicalSize<u32>),
    /// Move (physical pixels).
    SetPosition(PhysicalPosition<i32>),
    /// Show or hide.
    SetVisible(bool),
    /// Where the input method's composition and candidate windows open: the area of the
    /// text being composed, in physical pixels relative to the client area.
    SetImeCursorArea {
        /// Top-left of the area.
        position: PhysicalPosition<i32>,
        /// Size of the area. The candidate list opens just below it.
        size: PhysicalSize<u32>,
    },
    /// Ask for a `RedrawRequested` event.
    RequestRedraw,
    /// Set or clear (`None`) the minimum inner size, in physical pixels.
    SetMinSize(Option<PhysicalSize<u32>>),
    /// Set or clear (`None`) the maximum inner size, in physical pixels.
    SetMaxSize(Option<PhysicalSize<u32>>),
    /// Allow or forbid the user to resize the window.
    SetResizable(bool),
    /// Maximize (`true`) or restore (`false`).
    SetMaximized(bool),
    /// Minimize (`true`) or restore (`false`).
    SetMinimized(bool),
}

/// Creation parameters, built by [`WindowBuilder`](crate::WindowBuilder).
#[derive(Clone, Debug, PartialEq)]
pub struct WindowAttributes {
    /// Title.
    pub title: String,
    /// Initial inner size.
    pub inner_size: Option<LogicalSize<f64>>,
    /// Minimum size.
    pub min_size: Option<LogicalSize<f64>>,
    /// Maximum size.
    pub max_size: Option<LogicalSize<f64>>,
    /// Initial position.
    pub position: Option<LogicalPosition<f64>>,
    /// User can resize.
    pub resizable: bool,
    /// Initially visible.
    pub visible: bool,
    /// Show OS decorations.
    pub decorations: bool,
    /// Transparent framebuffer.
    pub transparent: bool,
    /// Vertical sync.
    pub vsync: bool,
    /// Capability token; if set, creation goes through the capability-gated path.
    pub capability: Option<CapabilityToken>,
}

impl Default for WindowAttributes {
    fn default() -> Self {
        Self {
            title: String::from("tpt-window"),
            inner_size: None,
            min_size: None,
            max_size: None,
            position: None,
            resizable: true,
            visible: true,
            decorations: true,
            transparent: false,
            vsync: true,
            capability: None,
        }
    }
}

/// Per-thread message pump implemented by each platform.
pub trait EventLoopBackend {
    /// Non-blocking: deliver pending OS events into `sink`.
    fn poll(&mut self, sink: &mut EventSink<'_>) -> PollStatus;
    /// Blocking: wait up to `timeout` for events, then deliver them.
    fn wait(&mut self, sink: &mut EventSink<'_>, timeout: Duration) -> PollStatus;
    /// Ask the loop to stop; the next poll reports [`PollStatus::Exit`].
    fn request_exit(&self);
    /// Create a window.
    fn create_window(
        &self,
        attrs: &WindowAttributes,
    ) -> Result<Arc<dyn WindowBackend>, WindowError>;
}

/// A platform window.
pub trait WindowBackend: Send + Sync {
    /// Identifier used in events.
    fn id(&self) -> WindowId;
    /// Shared state snapshot.
    fn state(&self) -> &Arc<SharedWindowState>;
    /// Native window handle (for wgpu surface creation).
    fn raw_window_handle(&self) -> Result<RawWindowHandle, WindowError>;
    /// Native display handle.
    fn raw_display_handle(&self) -> Result<RawDisplayHandle, WindowError>;
    /// Zero-copy surface handle, if the backend supports one.
    fn surface_handle(&self) -> Option<SurfaceHandle> {
        None
    }
    /// Queue a command; applied on the backend's next poll.
    fn send_command(&self, command: WindowCommand);
}
