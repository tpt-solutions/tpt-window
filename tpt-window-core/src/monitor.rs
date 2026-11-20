// SPDX-License-Identifier: MIT OR Apache-2.0
//! Information about the displays attached to the system.

use crate::dpi::{PhysicalPosition, PhysicalSize};

/// One attached display, as the system reports it right now.
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorInfo {
    /// The system's name for the display (for example `\.\DISPLAY1` on Windows), if known.
    pub name: Option<String>,
    /// Top-left corner of the display in the desktop's physical coordinates. Displays can have
    /// negative positions when they sit left of or above the primary one.
    pub position: PhysicalPosition<i32>,
    /// Size of the display in physical pixels.
    pub size: PhysicalSize<u32>,
    /// Scale factor (1.0 is 96 dpi).
    pub scale_factor: f64,
    /// Whether this is the primary display.
    pub primary: bool,
}
