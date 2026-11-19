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
#[rustfmt::skip]
pub enum KeyCode {
    /// The `A` letter key.
    A,
    /// The `B` letter key.
    B,
    /// The `C` letter key.
    C,
    /// The `D` letter key.
    D,
    /// The `E` letter key.
    E,
    /// The `F` letter key.
    F,
    /// The `G` letter key.
    G,
    /// The `H` letter key.
    H,
    /// The `I` letter key.
    I,
    /// The `J` letter key.
    J,
    /// The `K` letter key.
    K,
    /// The `L` letter key.
    L,
    /// The `M` letter key.
    M,
    /// The `N` letter key.
    N,
    /// The `O` letter key.
    O,
    /// The `P` letter key.
    P,
    /// The `Q` letter key.
    Q,
    /// The `R` letter key.
    R,
    /// The `S` letter key.
    S,
    /// The `T` letter key.
    T,
    /// The `U` letter key.
    U,
    /// The `V` letter key.
    V,
    /// The `W` letter key.
    W,
    /// The `X` letter key.
    X,
    /// The `Y` letter key.
    Y,
    /// The `Z` letter key.
    Z,
    /// The `0` key on the main row.
    Digit0,
    /// The `1` key on the main row.
    Digit1,
    /// The `2` key on the main row.
    Digit2,
    /// The `3` key on the main row.
    Digit3,
    /// The `4` key on the main row.
    Digit4,
    /// The `5` key on the main row.
    Digit5,
    /// The `6` key on the main row.
    Digit6,
    /// The `7` key on the main row.
    Digit7,
    /// The `8` key on the main row.
    Digit8,
    /// The `9` key on the main row.
    Digit9,
    /// Function key F1.
    F1,
    /// Function key F2.
    F2,
    /// Function key F3.
    F3,
    /// Function key F4.
    F4,
    /// Function key F5.
    F5,
    /// Function key F6.
    F6,
    /// Function key F7.
    F7,
    /// Function key F8.
    F8,
    /// Function key F9.
    F9,
    /// Function key F10.
    F10,
    /// Function key F11.
    F11,
    /// Function key F12.
    F12,
    /// Up arrow.
    ArrowUp,
    /// Down arrow.
    ArrowDown,
    /// Left arrow.
    ArrowLeft,
    /// Right arrow.
    ArrowRight,
    /// Escape.
    Escape,
    /// Enter / Return.
    Enter,
    /// Space bar.
    Space,
    /// Tab.
    Tab,
    /// Backspace.
    Backspace,
    /// Delete.
    Delete,
    /// Insert.
    Insert,
    /// Home.
    Home,
    /// End.
    End,
    /// Page Up.
    PageUp,
    /// Page Down.
    PageDown,
    /// Left Shift.
    ShiftLeft,
    /// Right Shift.
    ShiftRight,
    /// Left Control.
    ControlLeft,
    /// Right Control.
    ControlRight,
    /// Left Alt.
    AltLeft,
    /// Right Alt (AltGr on many layouts).
    AltRight,
    /// Left Super / Windows / Command key.
    SuperLeft,
    /// Right Super / Windows / Command key.
    SuperRight,
    /// Caps Lock.
    CapsLock,
    /// The `-` key on the main row.
    Minus,
    /// The `=` key on the main row.
    Equal,
    /// The `[` key.
    BracketLeft,
    /// The `]` key.
    BracketRight,
    /// The backslash key.
    Backslash,
    /// The `;` key.
    Semicolon,
    /// The apostrophe key.
    Quote,
    /// The grave accent / backtick key.
    Backquote,
    /// The `,` key.
    Comma,
    /// The `.` key.
    Period,
    /// The `/` key.
    Slash,
    /// Numpad `0`.
    Numpad0,
    /// Numpad `1`.
    Numpad1,
    /// Numpad `2`.
    Numpad2,
    /// Numpad `3`.
    Numpad3,
    /// Numpad `4`.
    Numpad4,
    /// Numpad `5`.
    Numpad5,
    /// Numpad `6`.
    Numpad6,
    /// Numpad `7`.
    Numpad7,
    /// Numpad `8`.
    Numpad8,
    /// Numpad `9`.
    Numpad9,
    /// Numpad `+`.
    NumpadAdd,
    /// Numpad `-`.
    NumpadSubtract,
    /// Numpad `*`.
    NumpadMultiply,
    /// Numpad `/`.
    NumpadDivide,
    /// Numpad Enter.
    NumpadEnter,
    /// Numpad decimal separator.
    NumpadDecimal,
    /// A key with no known code (the raw scancode is in `PhysicalKey::Unidentified`).
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

/// Maps an evdev key code (`KEY_*`) to a [`KeyCode`]. Codes name physical positions,
/// so the result does not depend on the keyboard layout.
pub fn key_from_evdev(code: u32) -> KeyCode {
    match code {
        1 => KeyCode::Escape,
        2 => KeyCode::Digit1,
        3 => KeyCode::Digit2,
        4 => KeyCode::Digit3,
        5 => KeyCode::Digit4,
        6 => KeyCode::Digit5,
        7 => KeyCode::Digit6,
        8 => KeyCode::Digit7,
        9 => KeyCode::Digit8,
        10 => KeyCode::Digit9,
        11 => KeyCode::Digit0,
        12 => KeyCode::Minus,
        13 => KeyCode::Equal,
        14 => KeyCode::Backspace,
        15 => KeyCode::Tab,
        16 => KeyCode::Q,
        17 => KeyCode::W,
        18 => KeyCode::E,
        19 => KeyCode::R,
        20 => KeyCode::T,
        21 => KeyCode::Y,
        22 => KeyCode::U,
        23 => KeyCode::I,
        24 => KeyCode::O,
        25 => KeyCode::P,
        26 => KeyCode::BracketLeft,
        27 => KeyCode::BracketRight,
        28 => KeyCode::Enter,
        29 => KeyCode::ControlLeft,
        30 => KeyCode::A,
        31 => KeyCode::S,
        32 => KeyCode::D,
        33 => KeyCode::F,
        34 => KeyCode::G,
        35 => KeyCode::H,
        36 => KeyCode::J,
        37 => KeyCode::K,
        38 => KeyCode::L,
        39 => KeyCode::Semicolon,
        40 => KeyCode::Quote,
        41 => KeyCode::Backquote,
        42 => KeyCode::ShiftLeft,
        43 => KeyCode::Backslash,
        44 => KeyCode::Z,
        45 => KeyCode::X,
        46 => KeyCode::C,
        47 => KeyCode::V,
        48 => KeyCode::B,
        49 => KeyCode::N,
        50 => KeyCode::M,
        51 => KeyCode::Comma,
        52 => KeyCode::Period,
        53 => KeyCode::Slash,
        54 => KeyCode::ShiftRight,
        55 => KeyCode::NumpadMultiply,
        56 => KeyCode::AltLeft,
        57 => KeyCode::Space,
        58 => KeyCode::CapsLock,
        59 => KeyCode::F1,
        60 => KeyCode::F2,
        61 => KeyCode::F3,
        62 => KeyCode::F4,
        63 => KeyCode::F5,
        64 => KeyCode::F6,
        65 => KeyCode::F7,
        66 => KeyCode::F8,
        67 => KeyCode::F9,
        68 => KeyCode::F10,
        71 => KeyCode::Numpad7,
        72 => KeyCode::Numpad8,
        73 => KeyCode::Numpad9,
        74 => KeyCode::NumpadSubtract,
        75 => KeyCode::Numpad4,
        76 => KeyCode::Numpad5,
        77 => KeyCode::Numpad6,
        78 => KeyCode::NumpadAdd,
        79 => KeyCode::Numpad1,
        80 => KeyCode::Numpad2,
        81 => KeyCode::Numpad3,
        82 => KeyCode::Numpad0,
        83 => KeyCode::NumpadDecimal,
        87 => KeyCode::F11,
        88 => KeyCode::F12,
        96 => KeyCode::NumpadEnter,
        97 => KeyCode::ControlRight,
        98 => KeyCode::NumpadDivide,
        100 => KeyCode::AltRight,
        102 => KeyCode::Home,
        103 => KeyCode::ArrowUp,
        104 => KeyCode::PageUp,
        105 => KeyCode::ArrowLeft,
        106 => KeyCode::ArrowRight,
        107 => KeyCode::End,
        108 => KeyCode::ArrowDown,
        109 => KeyCode::PageDown,
        110 => KeyCode::Insert,
        111 => KeyCode::Delete,
        125 => KeyCode::SuperLeft,
        126 => KeyCode::SuperRight,
        _ => KeyCode::Unidentified,
    }
}
