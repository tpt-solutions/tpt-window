// SPDX-License-Identifier: MIT OR Apache-2.0
//! Capability types.
//!
//! **Stubs.** tpt-archon's real capability type (`tpt-archon-bridge::capability`) is a
//! `(Resource, Right)` pair over pages, channels and devices; it has no window, input or
//! display scopes and no event signing. These types are tpt-window's own contract; the
//! `tpt-window-archon` crate will map them onto archon `Resource::Channel` / `Device`
//! capabilities once that design is settled.

/// Which windows a token may create or address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WindowScope {
    /// Only the window created with this token.
    Own,
    /// The window and its children.
    OwnAndChildren,
}

/// Which input events a window may receive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum InputScope {
    /// Only events targeted at this window.
    WindowOnly,
    /// Events targeted at this window and its children.
    WindowAndChildren,
    /// Global events (requires elevated privilege from tpt-archon).
    Global,
}

/// Which displays a window may be placed on.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DisplayScope {
    /// The display it was created on.
    Current,
    /// Any display.
    Any,
}

/// Capability presented to the display server / tpt-archon at window creation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CapabilityToken {
    /// Window scope.
    pub window_scope: WindowScope,
    /// Input scope.
    pub input_scope: InputScope,
    /// Display scope.
    pub display_scope: DisplayScope,
}

/// Opaque reference to a shared-memory frame surface (zero-copy sharing).
///
/// Stub: becomes a capability reference to a shared region in Phase 4.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SurfaceHandle {
    /// Backend-defined surface id.
    pub id: u64,
}
