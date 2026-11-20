// SPDX-License-Identifier: MIT OR Apache-2.0
//! [`Window`] handle and [`WindowBuilder`].

use std::sync::Arc;

use raw_window_handle::{RawDisplayHandle, RawWindowHandle};

use crate::backend::{WindowAttributes, WindowBackend, WindowCommand};
use crate::dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
use crate::error::WindowError;
use crate::event_loop::EventLoop;
use crate::ids::WindowId;
use crate::security::{CapabilityToken, SurfaceHandle};

/// Cheap-to-clone handle to a displayable surface.
#[derive(Clone)]
pub struct Window {
    id: WindowId,
    inner: Arc<dyn WindowBackend>,
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window").field("id", &self.id).finish()
    }
}

impl Window {
    pub(crate) fn from_backend(inner: Arc<dyn WindowBackend>) -> Self {
        Self {
            id: inner.id(),
            inner,
        }
    }

    /// Identifier used in events.
    pub fn id(&self) -> WindowId {
        self.id
    }

    /// Native window handle for wgpu surface creation.
    pub fn raw_window_handle(&self) -> Result<RawWindowHandle, WindowError> {
        self.inner.raw_window_handle()
    }

    /// Native display handle for wgpu surface creation.
    pub fn raw_display_handle(&self) -> Result<RawDisplayHandle, WindowError> {
        self.inner.raw_display_handle()
    }

    /// Zero-copy surface handle, if available.
    pub fn surface_handle(&self) -> Option<SurfaceHandle> {
        self.inner.surface_handle()
    }

    /// Inner size in physical pixels (cached; no syscall).
    pub fn size(&self) -> PhysicalSize<u32> {
        self.inner.state().snapshot().size
    }

    /// Inner size in logical units.
    pub fn logical_size(&self) -> LogicalSize<f64> {
        let s = self.inner.state().snapshot();
        s.size.to_logical(s.scale_factor)
    }

    /// DPI scale factor.
    pub fn scale_factor(&self) -> f64 {
        self.inner.state().snapshot().scale_factor
    }

    /// Outer position in physical pixels.
    pub fn position(&self) -> PhysicalPosition<i32> {
        self.inner.state().snapshot().position
    }

    /// Has input focus.
    pub fn is_focused(&self) -> bool {
        self.inner.state().snapshot().focused
    }

    /// Is visible.
    pub fn is_visible(&self) -> bool {
        self.inner.state().snapshot().visible
    }

    /// Set the title (applied on next poll).
    pub fn set_title(&self, title: &str) {
        self.inner
            .send_command(WindowCommand::SetTitle(title.to_owned()));
    }

    /// Resize (applied on next poll).
    pub fn set_size(&self, size: impl Into<crate::dpi::Size>) {
        let physical = size.into().to_physical(self.scale_factor());
        self.inner.send_command(WindowCommand::SetSize(physical));
    }

    /// Move (applied on next poll).
    pub fn set_position(&self, pos: impl Into<crate::dpi::Position>) {
        let physical = pos.into().to_physical(self.scale_factor());
        self.inner
            .send_command(WindowCommand::SetPosition(physical));
    }

    /// Show or hide (applied on next poll).
    pub fn set_visible(&self, visible: bool) {
        self.inner.send_command(WindowCommand::SetVisible(visible));
    }

    /// Places the input method's composition and candidate windows at the text being
    /// composed (physical pixels, relative to the client area). Applied on the next poll.
    ///
    /// Only Win32 acts on this so far. Other platforms accept the call and ignore it until
    /// their IME support lands (see todo.md).
    pub fn set_ime_cursor_area(&self, position: PhysicalPosition<i32>, size: PhysicalSize<u32>) {
        self.inner
            .send_command(WindowCommand::SetImeCursorArea { position, size });
    }

    /// Request a `RedrawRequested` event.
    pub fn request_redraw(&self) {
        self.inner.send_command(WindowCommand::RequestRedraw);
    }

    /// Sets or clears the minimum inner size (physical pixels). Applied on the next poll.
    pub fn set_min_size<S: Into<crate::dpi::Size>>(&self, size: Option<S>) {
        let physical = size.map(|s| s.into().to_physical(self.scale_factor()));
        self.inner.send_command(WindowCommand::SetMinSize(physical));
    }

    /// Sets or clears the maximum inner size (physical pixels). Applied on the next poll.
    pub fn set_max_size<S: Into<crate::dpi::Size>>(&self, size: Option<S>) {
        let physical = size.map(|s| s.into().to_physical(self.scale_factor()));
        self.inner.send_command(WindowCommand::SetMaxSize(physical));
    }

    /// Allows or forbids resizing by the user. Applied on the next poll.
    pub fn set_resizable(&self, resizable: bool) {
        self.inner
            .send_command(WindowCommand::SetResizable(resizable));
    }

    /// Maximizes or restores the window. Applied on the next poll.
    pub fn set_maximized(&self, maximized: bool) {
        self.inner
            .send_command(WindowCommand::SetMaximized(maximized));
    }

    /// Minimizes or restores the window. Applied on the next poll.
    pub fn set_minimized(&self, minimized: bool) {
        self.inner
            .send_command(WindowCommand::SetMinimized(minimized));
    }

    /// The display this window is on right now, if the platform can tell.
    pub fn current_monitor(&self) -> Option<crate::monitor::MonitorInfo> {
        self.inner.current_monitor()
    }

    /// The system's current light or dark appearance, if the platform reports one.
    pub fn theme(&self) -> Option<crate::theme::Theme> {
        self.inner.theme()
    }

    /// Shows or removes the title bar and border. The client area keeps its size. Applied on the
    /// next poll.
    pub fn set_decorations(&self, decorations: bool) {
        self.inner
            .send_command(WindowCommand::SetDecorations(decorations));
    }

    /// Covers the display the window is on (`true`), or restores the previous size (`false`).
    /// Applied on the next poll.
    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.inner
            .send_command(WindowCommand::SetFullscreen(fullscreen));
    }

    /// Keeps the window above other windows, or returns it to the normal order. Applied on the
    /// next poll.
    pub fn set_always_on_top(&self, on_top: bool) {
        self.inner
            .send_command(WindowCommand::SetAlwaysOnTop(on_top));
    }

    /// Confines or locks the pointer to this window. Applied on the next poll. Only Win32 acts on
    /// this so far; other platforms accept the call and ignore it.
    pub fn set_cursor_grab(&self, grab: crate::input::CursorGrab) {
        self.inner.send_command(WindowCommand::SetCursorGrab(grab));
    }

    /// Sets or clears the window icon. Applied on the next poll. Win32 and X11 show it; other
    /// platforms accept the call and ignore it.
    pub fn set_icon(&self, icon: Option<crate::icon::Icon>) {
        self.inner.send_command(WindowCommand::SetIcon(icon));
    }

    /// Shows or hides the mouse pointer over this window. Applied on the next poll. A hidden
    /// pointer still moves and still sends events; only its image is hidden.
    pub fn set_cursor_visible(&self, visible: bool) {
        self.inner
            .send_command(WindowCommand::SetCursorVisible(visible));
    }
}

/// Builder for [`Window`].
#[derive(Clone, Debug, Default)]
pub struct WindowBuilder {
    attrs: WindowAttributes,
}

impl WindowBuilder {
    /// Defaults: resizable, visible, decorated, vsync on.
    pub fn new() -> Self {
        Self::default()
    }

    /// Title.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.attrs.title = title.into();
        self
    }

    /// Initial inner size.
    pub fn with_inner_size(mut self, size: LogicalSize<f64>) -> Self {
        self.attrs.inner_size = Some(size);
        self
    }

    /// Minimum size.
    pub fn with_min_size(mut self, size: LogicalSize<f64>) -> Self {
        self.attrs.min_size = Some(size);
        self
    }

    /// Maximum size.
    pub fn with_max_size(mut self, size: LogicalSize<f64>) -> Self {
        self.attrs.max_size = Some(size);
        self
    }

    /// Initial position.
    pub fn with_position(mut self, pos: LogicalPosition<f64>) -> Self {
        self.attrs.position = Some(pos);
        self
    }

    /// User-resizable.
    pub fn with_resizable(mut self, resizable: bool) -> Self {
        self.attrs.resizable = resizable;
        self
    }

    /// Initially visible.
    pub fn with_visible(mut self, visible: bool) -> Self {
        self.attrs.visible = visible;
        self
    }

    /// OS decorations.
    pub fn with_decorations(mut self, decorations: bool) -> Self {
        self.attrs.decorations = decorations;
        self
    }

    /// Transparent framebuffer.
    pub fn with_transparent(mut self, transparent: bool) -> Self {
        self.attrs.transparent = transparent;
        self
    }

    /// Vertical sync.
    pub fn with_vsync(mut self, vsync: bool) -> Self {
        self.attrs.vsync = vsync;
        self
    }

    /// Create the window through the capability-gated path.
    pub fn with_capability(mut self, token: CapabilityToken) -> Self {
        self.attrs.capability = Some(token);
        self
    }

    /// The accumulated attributes.
    pub fn attributes(&self) -> &WindowAttributes {
        &self.attrs
    }

    /// Creates the window on `event_loop`.
    pub fn build(self, event_loop: &EventLoop) -> Result<Window, WindowError> {
        event_loop.create_window(&self.attrs)
    }
}
