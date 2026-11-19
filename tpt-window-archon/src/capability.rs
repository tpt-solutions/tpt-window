// SPDX-License-Identifier: MIT OR Apache-2.0
//! Mapping from tpt-window scopes onto archon capabilities.
//!
//! The types here mirror archon's `tpt-archon-bridge::capability::{Resource, Right}`
//! (same variants, same meaning) so that an archon adapter is a one-line delegation.
//!
//! | tpt-window                          | archon capability                          |
//! |-------------------------------------|--------------------------------------------|
//! | window `w` events                   | `Resource::Channel(w.0)`                   |
//! | receive events of `w` (owner)       | `Right::Read` on that channel              |
//! | deliver events to `w` (input source)| `Right::Write` on that channel             |
//! | `InputScope::Global`                | additionally `Right::Read` on `INPUT_DEVICE`   |
//! | `DisplayScope::Any`                 | additionally `Right::Read` on `DISPLAY_DEVICE` |
//!
//! `WindowScope::OwnAndChildren` and `InputScope::WindowAndChildren` are rejected with
//! [`ScopeError::Unsupported`]: routing a child's events to its parent is not designed
//! yet, and granting them silently would be a privilege escalation.

use tpt_window_core::{CapabilityToken, DisplayScope, InputScope, WindowId, WindowScope};

/// Device id for global input (archon `Resource::Device`).
pub const INPUT_DEVICE: u64 = 0;
/// Device id for displays that a window may be placed on any of.
pub const DISPLAY_DEVICE: u64 = 1;

/// What a capability refers to. Mirrors archon's `Resource` (only the variants used here).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Resource {
    /// An IPC channel. Each window has one, identified by its window id.
    Channel(u64),
    /// A hardware device.
    Device(u64),
}

/// What a capability permits. Mirrors archon's `Right`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Right {
    /// Read the resource.
    Read,
    /// Write the resource.
    Write,
    /// Read and write the resource.
    ReadWrite,
}

/// Answers whether a presented capability authorizes `right` on `resource`.
///
/// The implementation must also check liveness (a revoked token must fail), as archon's
/// `CapabilityIssuer::authorizes` does. A structural check alone is not enough.
pub trait Authority {
    /// The capability token the caller presents.
    type Token;

    /// Whether `token` is live and authorizes `right` on `resource`.
    fn authorizes(&self, token: &Self::Token, resource: Resource, right: Right) -> bool;
}

/// A scope the mapping cannot express yet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopeError {
    /// The scope is not supported; the message says which one.
    Unsupported(&'static str),
}

impl core::fmt::Display for ScopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            ScopeError::Unsupported(what) => write!(f, "unsupported scope: {what}"),
        }
    }
}

impl std::error::Error for ScopeError {}

/// The `(resource, right)` pairs a window's scopes require, without allocating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Requirements {
    items: [(Resource, Right); 3],
    len: usize,
}

impl Requirements {
    /// The required pairs, in a fixed order.
    pub fn as_slice(&self) -> &[(Resource, Right)] {
        &self.items[..self.len]
    }
}

/// Works out what a window with `token` needs, given its own id.
///
/// The owner always needs to read the window's channel. Global input and any-display
/// placement each add one device read.
pub fn requirements(token: &CapabilityToken, window: WindowId) -> Result<Requirements, ScopeError> {
    let mut req = Requirements {
        items: [(Resource::Channel(window.0), Right::Read); 3],
        len: 1,
    };
    match token.window_scope {
        WindowScope::Own => {}
        WindowScope::OwnAndChildren => {
            return Err(ScopeError::Unsupported(
                "WindowScope::OwnAndChildren (child routing is not designed yet)",
            ));
        }
        _ => {
            return Err(ScopeError::Unsupported("unknown WindowScope variant"));
        }
    }
    match token.input_scope {
        InputScope::WindowOnly => {}
        InputScope::Global => {
            req.items[req.len] = (Resource::Device(INPUT_DEVICE), Right::Read);
            req.len += 1;
        }
        InputScope::WindowAndChildren => {
            return Err(ScopeError::Unsupported(
                "InputScope::WindowAndChildren (child routing is not designed yet)",
            ));
        }
    }
    match token.display_scope {
        DisplayScope::Current => {}
        DisplayScope::Any => {
            req.items[req.len] = (Resource::Device(DISPLAY_DEVICE), Right::Read);
            req.len += 1;
        }
        _ => {
            return Err(ScopeError::Unsupported("unknown DisplayScope variant"));
        }
    }
    Ok(req)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(w: WindowScope, i: InputScope, d: DisplayScope) -> CapabilityToken {
        CapabilityToken {
            window_scope: w,
            input_scope: i,
            display_scope: d,
        }
    }

    #[test]
    fn plain_window_needs_only_its_channel() {
        let req = requirements(
            &token(
                WindowScope::Own,
                InputScope::WindowOnly,
                DisplayScope::Current,
            ),
            WindowId(5),
        )
        .unwrap();
        assert_eq!(req.as_slice(), &[(Resource::Channel(5), Right::Read)]);
    }

    #[test]
    fn global_input_and_any_display_add_device_reads() {
        let req = requirements(
            &token(WindowScope::Own, InputScope::Global, DisplayScope::Any),
            WindowId(2),
        )
        .unwrap();
        assert_eq!(
            req.as_slice(),
            &[
                (Resource::Channel(2), Right::Read),
                (Resource::Device(INPUT_DEVICE), Right::Read),
                (Resource::Device(DISPLAY_DEVICE), Right::Read),
            ]
        );
    }

    #[test]
    fn child_scopes_are_rejected_not_granted() {
        assert!(matches!(
            requirements(
                &token(
                    WindowScope::OwnAndChildren,
                    InputScope::WindowOnly,
                    DisplayScope::Current
                ),
                WindowId(1)
            ),
            Err(ScopeError::Unsupported(_))
        ));
        assert!(matches!(
            requirements(
                &token(
                    WindowScope::Own,
                    InputScope::WindowAndChildren,
                    DisplayScope::Current
                ),
                WindowId(1)
            ),
            Err(ScopeError::Unsupported(_))
        ));
    }
}
