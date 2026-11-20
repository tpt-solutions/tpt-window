// SPDX-License-Identifier: MIT OR Apache-2.0
//! Error type shared by all tpt-window crates.

use core::fmt;

/// Errors returned by window and event-loop operations.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum WindowError {
    /// The current platform or backend does not support the operation.
    Unsupported(&'static str),
    /// The operating system reported a failure.
    Os(String),
    /// The platform has a windowing system, but it is not running or cannot be reached (for
    /// example, `DISPLAY` is unset or the Wayland socket is missing). Try again in a session
    /// that has one.
    NoDisplay(String),
    /// The window has been destroyed.
    Destroyed,
    /// The capability token was rejected.
    CapabilityDenied,
    /// The window has no native handle (e.g. headless backend).
    NoNativeHandle,
}

impl fmt::Display for WindowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported(what) => write!(f, "unsupported: {what}"),
            Self::Os(msg) => write!(f, "os error: {msg}"),
            Self::NoDisplay(msg) => write!(f, "no display server: {msg}"),
            Self::Destroyed => f.write_str("window destroyed"),
            Self::CapabilityDenied => f.write_str("capability denied"),
            Self::NoNativeHandle => f.write_str("window has no native handle"),
        }
    }
}

impl std::error::Error for WindowError {}
