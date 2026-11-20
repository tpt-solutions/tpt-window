// SPDX-License-Identifier: MIT OR Apache-2.0
//! Layout-translated key codes: the key a press means on the user's layout.
//!
//! `KeyCode` is the meaning of a key (the letter A, the Enter key), and it follows the layout, as
//! on Win32 and the web. `PhysicalKey` is the position of the key, and it does not. A backend
//! reports the base meaning of the key here (the key with no shift applied), so that a shifted
//! `a` and `A` are both `KeyCode::A`, and `1` and `!` are both `KeyCode::Digit1`.
//!
//! The keysym values are X11's (`XK_*`), which xkbcommon uses too.

use crate::input::KeyCode;

/// The key an unshifted keysym names, or `None` when the keysym has no key code.
pub fn key_from_keysym(keysym: u32) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match keysym {
        // Letters: both cases name the same key.
        0x61..=0x7A => LETTERS[(keysym - 0x61) as usize],
        0x41..=0x5A => LETTERS[(keysym - 0x41) as usize],
        // Digits.
        0x30..=0x39 => DIGITS[(keysym - 0x30) as usize],
        // Punctuation, as the ASCII code of the unshifted symbol.
        0x20 => Space,
        0x2D => Minus,
        0x3D => Equal,
        0x5B => BracketLeft,
        0x5D => BracketRight,
        0x5C => Backslash,
        0x3B => Semicolon,
        0x27 => Quote,
        0x60 => Backquote,
        0x2C => Comma,
        0x2E => Period,
        0x2F => Slash,
        // The key beside the left Shift on ISO keyboards.
        0x3C | 0x3E => IntlBackslash,
        // Editing and navigation.
        0xFF08 => Backspace,
        0xFF09 => Tab,
        0xFF0D => Enter,
        0xFF1B => Escape,
        0xFFFF => Delete,
        0xFF63 => Insert,
        0xFF50 => Home,
        0xFF57 => End,
        0xFF55 => PageUp,
        0xFF56 => PageDown,
        0xFF51 => ArrowLeft,
        0xFF52 => ArrowUp,
        0xFF53 => ArrowRight,
        0xFF54 => ArrowDown,
        // Function keys.
        0xFFBE..=0xFFC9 => FKEYS[(keysym - 0xFFBE) as usize],
        0xFFCA..=0xFFD5 => F13_F24[(keysym - 0xFFCA) as usize],
        // Modifiers and locks.
        0xFFE1 => ShiftLeft,
        0xFFE2 => ShiftRight,
        0xFFE3 => ControlLeft,
        0xFFE4 => ControlRight,
        0xFFE9 => AltLeft,
        0xFFEA => AltRight,
        0xFFEB => SuperLeft,
        0xFFEC => SuperRight,
        0xFFE5 => CapsLock,
        0xFF7F => NumLock,
        0xFF14 => ScrollLock,
        0xFF61 => PrintScreen,
        0xFF13 => Pause,
        0xFF67 => Menu,
        // Keypad.
        0xFFB0..=0xFFB9 => NUMPAD[(keysym - 0xFFB0) as usize],
        0xFFAA => NumpadMultiply,
        0xFFAB => NumpadAdd,
        0xFFAD => NumpadSubtract,
        0xFFAE => NumpadDecimal,
        0xFFAF => NumpadDivide,
        0xFF8D => NumpadEnter,
        // Media and volume keys (the XF86 keysyms).
        0x1008_FF14 => MediaPlayPause,
        0x1008_FF15 => MediaStop,
        0x1008_FF16 => MediaTrackPrevious,
        0x1008_FF17 => MediaTrackNext,
        0x1008_FF11 => AudioVolumeDown,
        0x1008_FF12 => AudioVolumeMute,
        0x1008_FF13 => AudioVolumeUp,
        _ => return None,
    })
}

/// Like [`key_from_keysym`], but for a keysym that already has Shift applied, as sctk reports it
/// on Wayland. A shifted symbol names the key it is printed on, as on a US layout, so `!` is the
/// `1` key. This is an approximation: on other layouts the shifted symbols sit on other keys,
/// and those keys report the unshifted meaning where they can.
pub fn key_from_shifted_keysym(keysym: u32) -> Option<KeyCode> {
    if let Some(key) = key_from_keysym(keysym) {
        return Some(key);
    }
    key_from_char(char::from_u32(keysym)?)
}

/// The key a character is printed on, on a US layout, for the characters that have one.
/// Shifted symbols map to the key beneath them. Letters of either case name the same key.
pub fn key_from_char(c: char) -> Option<KeyCode> {
    use KeyCode::*;
    Some(match c {
        'a'..='z' => LETTERS[(c as u8 - b'a') as usize],
        'A'..='Z' => LETTERS[(c as u8 - b'A') as usize],
        '0'..='9' => DIGITS[(c as u8 - b'0') as usize],
        ')' => Digit0,
        '!' => Digit1,
        '@' => Digit2,
        '#' => Digit3,
        '$' => Digit4,
        '%' => Digit5,
        '^' => Digit6,
        '&' => Digit7,
        '*' => Digit8,
        '(' => Digit9,
        ' ' => Space,
        '-' | '_' => Minus,
        '=' | '+' => Equal,
        '[' | '{' => BracketLeft,
        ']' | '}' => BracketRight,
        '\\' | '|' => Backslash,
        ';' | ':' => Semicolon,
        '\'' | '"' => Quote,
        '`' | '~' => Backquote,
        ',' | '<' => Comma,
        '.' | '>' => Period,
        '/' | '?' => Slash,
        '\t' => Tab,
        '\r' | '\n' => Enter,
        '\u{1B}' => Escape,
        '\u{8}' | '\u{7F}' => Backspace,
        _ => return None,
    })
}

const LETTERS: [KeyCode; 26] = {
    use KeyCode::*;
    [
        A, B, C, D, E, F, G, H, I, J, K, L, M, N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    ]
};

const DIGITS: [KeyCode; 10] = {
    use KeyCode::*;
    [
        Digit0, Digit1, Digit2, Digit3, Digit4, Digit5, Digit6, Digit7, Digit8, Digit9,
    ]
};

const NUMPAD: [KeyCode; 10] = {
    use KeyCode::*;
    [
        Numpad0, Numpad1, Numpad2, Numpad3, Numpad4, Numpad5, Numpad6, Numpad7, Numpad8, Numpad9,
    ]
};

const FKEYS: [KeyCode; 12] = {
    use KeyCode::*;
    [F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12]
};

const F13_F24: [KeyCode; 12] = {
    use KeyCode::*;
    [F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23, F24]
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_cases_of_a_letter_are_the_same_key() {
        assert_eq!(key_from_keysym(0x61), Some(KeyCode::A));
        assert_eq!(key_from_keysym(0x41), Some(KeyCode::A));
        assert_eq!(key_from_keysym(0x7A), Some(KeyCode::Z));
    }

    #[test]
    fn digits_punctuation_and_function_keys_map() {
        assert_eq!(key_from_keysym(0x31), Some(KeyCode::Digit1));
        assert_eq!(key_from_keysym(0x2D), Some(KeyCode::Minus));
        assert_eq!(key_from_keysym(0xFFBE), Some(KeyCode::F1));
        assert_eq!(key_from_keysym(0xFFC9), Some(KeyCode::F12));
        assert_eq!(key_from_keysym(0xFFCA), Some(KeyCode::F13));
        assert_eq!(key_from_keysym(0xFFD5), Some(KeyCode::F24));
        assert_eq!(key_from_keysym(0xFFB5), Some(KeyCode::Numpad5));
    }

    #[test]
    fn media_and_lock_keys_map() {
        assert_eq!(key_from_keysym(0x1008_FF14), Some(KeyCode::MediaPlayPause));
        assert_eq!(key_from_keysym(0x1008_FF13), Some(KeyCode::AudioVolumeUp));
        assert_eq!(key_from_keysym(0xFF7F), Some(KeyCode::NumLock));
    }

    #[test]
    fn dead_keys_and_unknown_keysyms_have_no_key() {
        assert_eq!(key_from_keysym(0xFE50), None, "dead grave");
        assert_eq!(key_from_keysym(0xE9), None, "é is a character, not a key");
    }

    #[test]
    fn shifted_symbols_name_the_key_they_are_printed_on() {
        assert_eq!(key_from_shifted_keysym(0x21), Some(KeyCode::Digit1));
        assert_eq!(key_from_shifted_keysym(0x41), Some(KeyCode::A));
        assert_eq!(key_from_shifted_keysym(0xFF0D), Some(KeyCode::Enter));
        assert_eq!(key_from_shifted_keysym(0x3F), Some(KeyCode::Slash));
    }

    #[test]
    fn characters_map_to_their_keys() {
        assert_eq!(key_from_char('q'), Some(KeyCode::Q));
        assert_eq!(key_from_char('\r'), Some(KeyCode::Enter));
        assert_eq!(key_from_char('\u{1B}'), Some(KeyCode::Escape));
        assert_eq!(key_from_char('\u{F704}'), None, "private-use function key");
    }
}
