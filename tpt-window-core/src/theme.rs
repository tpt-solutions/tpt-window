// SPDX-License-Identifier: MIT OR Apache-2.0
//! The light or dark appearance the user chose for the system.

/// The system's light or dark appearance.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Theme {
    /// Light appearance.
    Light,
    /// Dark appearance.
    Dark,
}
