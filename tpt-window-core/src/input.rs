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
    /// The Num Lock key.
    NumLock,
    /// The Scroll Lock key.
    ScrollLock,
    /// The Print Screen (SysRq) key.
    PrintScreen,
    /// The Pause (Break) key.
    Pause,
    /// The application menu key (between the right Super and Control keys).
    Menu,
    /// Function key F13.
    F13,
    /// Function key F14.
    F14,
    /// Function key F15.
    F15,
    /// Function key F16.
    F16,
    /// Function key F17.
    F17,
    /// Function key F18.
    F18,
    /// Function key F19.
    F19,
    /// Function key F20.
    F20,
    /// Function key F21.
    F21,
    /// Function key F22.
    F22,
    /// Function key F23.
    F23,
    /// Function key F24.
    F24,
    /// The extra key next to the left Shift on ISO keyboards.
    IntlBackslash,
    /// The media play/pause key.
    MediaPlayPause,
    /// The media stop key.
    MediaStop,
    /// The media next-track key.
    MediaTrackNext,
    /// The media previous-track key.
    MediaTrackPrevious,
    /// The volume up key.
    AudioVolumeUp,
    /// The volume down key.
    AudioVolumeDown,
    /// The volume mute key.
    AudioVolumeMute,
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
        69 => KeyCode::NumLock,
        70 => KeyCode::ScrollLock,
        99 => KeyCode::PrintScreen,
        119 => KeyCode::Pause,
        127 => KeyCode::Menu,
        183 => KeyCode::F13,
        184 => KeyCode::F14,
        185 => KeyCode::F15,
        186 => KeyCode::F16,
        187 => KeyCode::F17,
        188 => KeyCode::F18,
        189 => KeyCode::F19,
        190 => KeyCode::F20,
        191 => KeyCode::F21,
        192 => KeyCode::F22,
        193 => KeyCode::F23,
        194 => KeyCode::F24,
        86 => KeyCode::IntlBackslash,
        164 => KeyCode::MediaPlayPause,
        166 => KeyCode::MediaStop,
        163 => KeyCode::MediaTrackNext,
        165 => KeyCode::MediaTrackPrevious,
        115 => KeyCode::AudioVolumeUp,
        114 => KeyCode::AudioVolumeDown,
        113 => KeyCode::AudioVolumeMute,
        _ => KeyCode::Unidentified,
    }
}

/// The modifier keys held down. A plain bit set, so it is `Copy` and small enough for an event.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ModifiersState(u8);

impl ModifiersState {
    const SHIFT: u8 = 1;
    const CONTROL: u8 = 2;
    const ALT: u8 = 4;
    const SUPER: u8 = 8;

    /// No modifiers held.
    pub const fn empty() -> Self {
        Self(0)
    }

    /// The raw bits, for encoding. See [`ModifiersState::from_bits`] for the inverse.
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Rebuilds the state from [`bits`](Self::bits). Bits outside the four modifiers are dropped.
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits & 0x0F)
    }

    /// Builds the state from the flags of the individual keys.
    pub const fn new(shift: bool, control: bool, alt: bool, super_key: bool) -> Self {
        const fn bit(on: bool, flag: u8) -> u8 {
            if on { flag } else { 0 }
        }
        Self(
            bit(shift, Self::SHIFT)
                | bit(control, Self::CONTROL)
                | bit(alt, Self::ALT)
                | bit(super_key, Self::SUPER),
        )
    }

    /// Either Shift key.
    pub const fn shift(self) -> bool {
        self.0 & Self::SHIFT != 0
    }

    /// Either Control key.
    pub const fn control(self) -> bool {
        self.0 & Self::CONTROL != 0
    }

    /// Either Alt key (Option on macOS).
    pub const fn alt(self) -> bool {
        self.0 & Self::ALT != 0
    }

    /// Either Super key (Windows key, Command on macOS).
    pub const fn super_key(self) -> bool {
        self.0 & Self::SUPER != 0
    }
}

#[cfg(test)]
mod modifier_tests {
    use super::ModifiersState;

    #[test]
    fn flags_round_trip_through_bits() {
        let m = ModifiersState::new(true, false, true, false);
        assert!(m.shift() && !m.control() && m.alt() && !m.super_key());
        assert_eq!(ModifiersState::from_bits(m.bits()), m);
        assert_eq!(
            ModifiersState::empty(),
            ModifiersState::new(false, false, false, false)
        );
    }

    #[test]
    fn from_bits_drops_unknown_bits() {
        assert_eq!(ModifiersState::from_bits(0xF0), ModifiersState::empty());
    }
}

#[cfg(test)]
mod evdev_tests {
    use super::{KeyCode, key_from_evdev};

    #[test]
    fn extended_keys_map_from_evdev() {
        assert_eq!(key_from_evdev(69), KeyCode::NumLock);
        assert_eq!(key_from_evdev(99), KeyCode::PrintScreen);
        assert_eq!(key_from_evdev(183), KeyCode::F13);
        assert_eq!(key_from_evdev(194), KeyCode::F24);
        assert_eq!(key_from_evdev(113), KeyCode::AudioVolumeMute);
        assert_eq!(key_from_evdev(4000), KeyCode::Unidentified);
    }
}
