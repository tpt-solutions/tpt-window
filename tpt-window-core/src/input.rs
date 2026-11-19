// SPDX-License-Identifier: MIT OR Apache-2.0
//! Input enums and small `Copy` payloads.

use crate::dpi::PhysicalPosition;

/// Pressed or released.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementState {
    /// Key/button went down.
    Pressed,
    /// Key/button went up.
    Released,
}

/// Mouse buttons.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Primary button.
    Left,
    /// Secondary button.
    Right,
    /// Middle / wheel button.
    Middle,
    /// "Back" side button.
    Back,
    /// "Forward" side button.
    Forward,
    /// Any other button.
    Other(u16),
}

/// Scroll amount.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MouseScrollDelta {
    /// Lines (notched wheel).
    LineDelta {
        /// Horizontal lines.
        x: f32,
        /// Vertical lines.
        y: f32,
    },
    /// Pixels (precision touchpad).
    PixelDelta {
        /// Horizontal pixels.
        x: f64,
        /// Vertical pixels.
        y: f64,
    },
}

/// Phase of a touch or scroll gesture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TouchPhase {
    /// Gesture began.
    Started,
    /// Gesture moved.
    Moved,
    /// Gesture ended normally.
    Ended,
    /// Gesture was cancelled.
    Cancelled,
}

/// A single touch point update.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TouchEvent {
    /// Identifier of the finger/pointer.
    pub id: u64,
    /// Phase.
    pub phase: TouchPhase,
    /// Position in physical pixels.
    pub position: PhysicalPosition<f64>,
    /// Normalised force 0.0..=1.0 (0.0 when unsupported).
    pub force: f32,
}

/// Identifier of a connected gamepad.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GamepadId(pub u32);

/// Key identifiers (W3C `code`-style names for physical keys).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
#[allow(missing_docs)]
#[rustfmt::skip]
pub enum KeyCode {
    A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    Escape, Enter, Space, Tab, Backspace, Delete, Insert, Home, End, PageUp, PageDown,
    ShiftLeft, ShiftRight, ControlLeft, ControlRight, AltLeft, AltRight, SuperLeft, SuperRight,
    CapsLock,
    Minus, Equal, BracketLeft, BracketRight, Backslash, Semicolon, Quote, Backquote,
    Comma, Period, Slash,
    Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    NumpadAdd, NumpadSubtract, NumpadMultiply, NumpadDivide, NumpadEnter, NumpadDecimal,
    /// Key with no mapping; see [`PhysicalKey::Unidentified`] for the native code.
    Unidentified,
}

/// Hardware key identity independent of layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PhysicalKey {
    /// A known physical key position.
    Code(KeyCode),
    /// Platform scancode we could not map.
    Unidentified(u32),
}
