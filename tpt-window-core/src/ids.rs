// SPDX-License-Identifier: MIT OR Apache-2.0
//! Identifiers.

/// Opaque identifier of a window within one event loop.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u64);

impl WindowId {
    /// Pseudo-id for events that are not tied to a window (e.g. yield points).
    pub const NONE: WindowId = WindowId(0);
}
