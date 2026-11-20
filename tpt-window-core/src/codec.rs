// SPDX-License-Identifier: MIT OR Apache-2.0
//! The canonical binary encoding of window events.
//!
//! Recordings and signed logs store events in this form, and archon signs it, so the bytes must
//! stay the same from release to release. Every variant has an explicit tag, and key codes have
//! explicit numbers (see [`key_tag`]), so reordering an enum cannot change what is stored.
//! Floats are stored by bit pattern.
//!
//! The encoder and decoder match exhaustively on `WindowEvent`, so a new variant fails to compile
//! until it is handled here.

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::event::{Event, WindowEvent};
use crate::ids::WindowId;
use crate::input::{
    ElementState, GamepadId, KeyCode, ModifiersState, MouseButton, MouseScrollDelta, PhysicalKey,
    TouchEvent, TouchPhase,
};
use crate::text::{ImeEvent, ImeText};
use crate::theme::Theme;

/// Stable wire number for a key. The match is exhaustive: a new `KeyCode` variant fails to compile
/// here until it is given a number, so no key can share a number or fall back to a default. These numbers are part of stored and signed bytes, so they are
/// written out here and never derived from the enum's declaration order.
pub fn key_tag(k: KeyCode) -> u32 {
    match k {
        KeyCode::A => 0,
        KeyCode::B => 1,
        KeyCode::C => 2,
        KeyCode::D => 3,
        KeyCode::E => 4,
        KeyCode::F => 5,
        KeyCode::G => 6,
        KeyCode::H => 7,
        KeyCode::I => 8,
        KeyCode::J => 9,
        KeyCode::K => 10,
        KeyCode::L => 11,
        KeyCode::M => 12,
        KeyCode::N => 13,
        KeyCode::O => 14,
        KeyCode::P => 15,
        KeyCode::Q => 16,
        KeyCode::R => 17,
        KeyCode::S => 18,
        KeyCode::T => 19,
        KeyCode::U => 20,
        KeyCode::V => 21,
        KeyCode::W => 22,
        KeyCode::X => 23,
        KeyCode::Y => 24,
        KeyCode::Z => 25,
        KeyCode::Digit0 => 26,
        KeyCode::Digit1 => 27,
        KeyCode::Digit2 => 28,
        KeyCode::Digit3 => 29,
        KeyCode::Digit4 => 30,
        KeyCode::Digit5 => 31,
        KeyCode::Digit6 => 32,
        KeyCode::Digit7 => 33,
        KeyCode::Digit8 => 34,
        KeyCode::Digit9 => 35,
        KeyCode::F1 => 36,
        KeyCode::F2 => 37,
        KeyCode::F3 => 38,
        KeyCode::F4 => 39,
        KeyCode::F5 => 40,
        KeyCode::F6 => 41,
        KeyCode::F7 => 42,
        KeyCode::F8 => 43,
        KeyCode::F9 => 44,
        KeyCode::F10 => 45,
        KeyCode::F11 => 46,
        KeyCode::F12 => 47,
        KeyCode::ArrowUp => 48,
        KeyCode::ArrowDown => 49,
        KeyCode::ArrowLeft => 50,
        KeyCode::ArrowRight => 51,
        KeyCode::Escape => 52,
        KeyCode::Enter => 53,
        KeyCode::Space => 54,
        KeyCode::Tab => 55,
        KeyCode::Backspace => 56,
        KeyCode::Delete => 57,
        KeyCode::Insert => 58,
        KeyCode::Home => 59,
        KeyCode::End => 60,
        KeyCode::PageUp => 61,
        KeyCode::PageDown => 62,
        KeyCode::ShiftLeft => 63,
        KeyCode::ShiftRight => 64,
        KeyCode::ControlLeft => 65,
        KeyCode::ControlRight => 66,
        KeyCode::AltLeft => 67,
        KeyCode::AltRight => 68,
        KeyCode::SuperLeft => 69,
        KeyCode::SuperRight => 70,
        KeyCode::CapsLock => 71,
        KeyCode::Minus => 72,
        KeyCode::Equal => 73,
        KeyCode::BracketLeft => 74,
        KeyCode::BracketRight => 75,
        KeyCode::Backslash => 76,
        KeyCode::Semicolon => 77,
        KeyCode::Quote => 78,
        KeyCode::Backquote => 79,
        KeyCode::Comma => 80,
        KeyCode::Period => 81,
        KeyCode::Slash => 82,
        KeyCode::Numpad0 => 83,
        KeyCode::Numpad1 => 84,
        KeyCode::Numpad2 => 85,
        KeyCode::Numpad3 => 86,
        KeyCode::Numpad4 => 87,
        KeyCode::Numpad5 => 88,
        KeyCode::Numpad6 => 89,
        KeyCode::Numpad7 => 90,
        KeyCode::Numpad8 => 91,
        KeyCode::Numpad9 => 92,
        KeyCode::NumpadAdd => 93,
        KeyCode::NumpadSubtract => 94,
        KeyCode::NumpadMultiply => 95,
        KeyCode::NumpadDivide => 96,
        KeyCode::NumpadEnter => 97,
        KeyCode::NumpadDecimal => 98,
        KeyCode::Unidentified => 99,
        KeyCode::NumLock => 100,
        KeyCode::ScrollLock => 101,
        KeyCode::PrintScreen => 102,
        KeyCode::Pause => 103,
        KeyCode::Menu => 104,
        KeyCode::F13 => 105,
        KeyCode::F14 => 106,
        KeyCode::F15 => 107,
        KeyCode::F16 => 108,
        KeyCode::F17 => 109,
        KeyCode::F18 => 110,
        KeyCode::F19 => 111,
        KeyCode::F20 => 112,
        KeyCode::F21 => 113,
        KeyCode::F22 => 114,
        KeyCode::F23 => 115,
        KeyCode::F24 => 116,
        KeyCode::IntlBackslash => 117,
        KeyCode::MediaPlayPause => 118,
        KeyCode::MediaStop => 119,
        KeyCode::MediaTrackNext => 120,
        KeyCode::MediaTrackPrevious => 121,
        KeyCode::AudioVolumeUp => 122,
        KeyCode::AudioVolumeDown => 123,
        KeyCode::AudioVolumeMute => 124,
    }
}

/// The key a wire number stands for, or `None` for a number this version does not know.
pub fn key_from_tag(tag: u32) -> Option<KeyCode> {
    KEYS_BY_TAG.get(tag as usize).copied()
}

/// Every known key, indexed by its wire number.
const KEYS_BY_TAG: [KeyCode; 125] = [
    KeyCode::A,
    KeyCode::B,
    KeyCode::C,
    KeyCode::D,
    KeyCode::E,
    KeyCode::F,
    KeyCode::G,
    KeyCode::H,
    KeyCode::I,
    KeyCode::J,
    KeyCode::K,
    KeyCode::L,
    KeyCode::M,
    KeyCode::N,
    KeyCode::O,
    KeyCode::P,
    KeyCode::Q,
    KeyCode::R,
    KeyCode::S,
    KeyCode::T,
    KeyCode::U,
    KeyCode::V,
    KeyCode::W,
    KeyCode::X,
    KeyCode::Y,
    KeyCode::Z,
    KeyCode::Digit0,
    KeyCode::Digit1,
    KeyCode::Digit2,
    KeyCode::Digit3,
    KeyCode::Digit4,
    KeyCode::Digit5,
    KeyCode::Digit6,
    KeyCode::Digit7,
    KeyCode::Digit8,
    KeyCode::Digit9,
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
    KeyCode::F12,
    KeyCode::ArrowUp,
    KeyCode::ArrowDown,
    KeyCode::ArrowLeft,
    KeyCode::ArrowRight,
    KeyCode::Escape,
    KeyCode::Enter,
    KeyCode::Space,
    KeyCode::Tab,
    KeyCode::Backspace,
    KeyCode::Delete,
    KeyCode::Insert,
    KeyCode::Home,
    KeyCode::End,
    KeyCode::PageUp,
    KeyCode::PageDown,
    KeyCode::ShiftLeft,
    KeyCode::ShiftRight,
    KeyCode::ControlLeft,
    KeyCode::ControlRight,
    KeyCode::AltLeft,
    KeyCode::AltRight,
    KeyCode::SuperLeft,
    KeyCode::SuperRight,
    KeyCode::CapsLock,
    KeyCode::Minus,
    KeyCode::Equal,
    KeyCode::BracketLeft,
    KeyCode::BracketRight,
    KeyCode::Backslash,
    KeyCode::Semicolon,
    KeyCode::Quote,
    KeyCode::Backquote,
    KeyCode::Comma,
    KeyCode::Period,
    KeyCode::Slash,
    KeyCode::Numpad0,
    KeyCode::Numpad1,
    KeyCode::Numpad2,
    KeyCode::Numpad3,
    KeyCode::Numpad4,
    KeyCode::Numpad5,
    KeyCode::Numpad6,
    KeyCode::Numpad7,
    KeyCode::Numpad8,
    KeyCode::Numpad9,
    KeyCode::NumpadAdd,
    KeyCode::NumpadSubtract,
    KeyCode::NumpadMultiply,
    KeyCode::NumpadDivide,
    KeyCode::NumpadEnter,
    KeyCode::NumpadDecimal,
    KeyCode::Unidentified,
    KeyCode::NumLock,
    KeyCode::ScrollLock,
    KeyCode::PrintScreen,
    KeyCode::Pause,
    KeyCode::Menu,
    KeyCode::F13,
    KeyCode::F14,
    KeyCode::F15,
    KeyCode::F16,
    KeyCode::F17,
    KeyCode::F18,
    KeyCode::F19,
    KeyCode::F20,
    KeyCode::F21,
    KeyCode::F22,
    KeyCode::F23,
    KeyCode::F24,
    KeyCode::IntlBackslash,
    KeyCode::MediaPlayPause,
    KeyCode::MediaStop,
    KeyCode::MediaTrackNext,
    KeyCode::MediaTrackPrevious,
    KeyCode::AudioVolumeUp,
    KeyCode::AudioVolumeDown,
    KeyCode::AudioVolumeMute,
];

/// Why bytes could not be decoded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CodecError {
    /// The bytes ended before the value did.
    Truncated,
    /// A tag byte names no variant.
    BadTag(u8),
    /// A value is out of range for its field (a key number, a bool byte, a path chunk that is
    /// too long).
    BadValue,
}

impl core::fmt::Display for CodecError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CodecError::Truncated => f.write_str("event bytes end early"),
            CodecError::BadTag(t) => write!(f, "unknown event tag {t}"),
            CodecError::BadValue => f.write_str("event field out of range"),
        }
    }
}

impl std::error::Error for CodecError {}

// ---------------------------------------------------------------------------
// Encoding
// ---------------------------------------------------------------------------

fn put_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
}
fn put_u16(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_u32(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_i32(out: &mut Vec<u8>, v: i32) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_u64(out: &mut Vec<u8>, v: u64) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn put_f32(out: &mut Vec<u8>, v: f32) {
    put_u32(out, v.to_bits());
}
fn put_f64(out: &mut Vec<u8>, v: f64) {
    put_u64(out, v.to_bits());
}
fn put_bool(out: &mut Vec<u8>, v: bool) {
    put_u8(out, u8::from(v));
}
fn put_str(out: &mut Vec<u8>, s: &str) {
    put_u32(out, s.len() as u32);
    out.extend_from_slice(s.as_bytes());
}
fn put_state(out: &mut Vec<u8>, s: ElementState) {
    put_u8(
        out,
        match s {
            ElementState::Pressed => 0,
            ElementState::Released => 1,
        },
    );
}
fn put_button(out: &mut Vec<u8>, b: MouseButton) {
    match b {
        MouseButton::Left => put_u8(out, 0),
        MouseButton::Right => put_u8(out, 1),
        MouseButton::Middle => put_u8(out, 2),
        MouseButton::Back => put_u8(out, 3),
        MouseButton::Forward => put_u8(out, 4),
        MouseButton::Other(n) => {
            put_u8(out, 5);
            put_u16(out, n);
        }
    }
}
fn put_phase(out: &mut Vec<u8>, p: TouchPhase) {
    put_u8(
        out,
        match p {
            TouchPhase::Started => 0,
            TouchPhase::Moved => 1,
            TouchPhase::Ended => 2,
            TouchPhase::Cancelled => 3,
        },
    );
}
fn put_key(out: &mut Vec<u8>, k: KeyCode) {
    put_u32(out, key_tag(k));
}
fn put_physical(out: &mut Vec<u8>, k: PhysicalKey) {
    match k {
        PhysicalKey::Code(code) => {
            put_u8(out, 0);
            put_key(out, code);
        }
        PhysicalKey::Unidentified(raw) => {
            put_u8(out, 1);
            put_u32(out, raw);
        }
    }
}
fn put_ime(out: &mut Vec<u8>, ime: &ImeEvent) {
    match ime {
        ImeEvent::Enabled => put_u8(out, 0),
        ImeEvent::Preedit { text, continued } => {
            put_u8(out, 1);
            put_str(out, text.as_str());
            put_bool(out, *continued);
        }
        ImeEvent::Commit { text, continued } => {
            put_u8(out, 2);
            put_str(out, text.as_str());
            put_bool(out, *continued);
        }
        ImeEvent::Disabled => put_u8(out, 3),
    }
}
fn put_wheel(out: &mut Vec<u8>, d: MouseScrollDelta) {
    match d {
        MouseScrollDelta::LineDelta { x, y } => {
            put_u8(out, 0);
            put_f32(out, x);
            put_f32(out, y);
        }
        MouseScrollDelta::PixelDelta { x, y } => {
            put_u8(out, 1);
            put_f64(out, x);
            put_f64(out, y);
        }
    }
}
fn put_touch(out: &mut Vec<u8>, t: &TouchEvent) {
    put_u64(out, t.id);
    put_phase(out, t.phase);
    put_f64(out, t.position.x);
    put_f64(out, t.position.y);
    put_f32(out, t.force);
}

/// Appends the canonical encoding of `event` to `out`.
pub fn encode_event(event: &WindowEvent, out: &mut Vec<u8>) {
    match event {
        WindowEvent::CloseRequested => put_u8(out, 0),
        WindowEvent::Destroyed => put_u8(out, 1),
        WindowEvent::Resized(PhysicalSize { width, height }) => {
            put_u8(out, 2);
            put_u32(out, *width);
            put_u32(out, *height);
        }
        WindowEvent::Moved(PhysicalPosition { x, y }) => {
            put_u8(out, 3);
            put_i32(out, *x);
            put_i32(out, *y);
        }
        WindowEvent::ScaleFactorChanged {
            scale_factor,
            new_inner_size,
        } => {
            put_u8(out, 4);
            put_f64(out, *scale_factor);
            put_u32(out, new_inner_size.width);
            put_u32(out, new_inner_size.height);
        }
        WindowEvent::Focused(v) => {
            put_u8(out, 5);
            put_bool(out, *v);
        }
        WindowEvent::Occluded(v) => {
            put_u8(out, 6);
            put_bool(out, *v);
        }
        WindowEvent::KeyboardInput {
            state,
            key_code,
            physical_key,
        } => {
            put_u8(out, 7);
            put_state(out, *state);
            put_key(out, *key_code);
            put_physical(out, *physical_key);
        }
        WindowEvent::Ime(ime) => {
            put_u8(out, 8);
            put_ime(out, ime);
        }
        WindowEvent::CursorMoved { position, pressure } => {
            put_u8(out, 9);
            put_f64(out, position.x);
            put_f64(out, position.y);
            put_f32(out, *pressure);
        }
        WindowEvent::CursorEntered => put_u8(out, 10),
        WindowEvent::CursorLeft => put_u8(out, 11),
        WindowEvent::MouseInput { state, button } => {
            put_u8(out, 12);
            put_state(out, *state);
            put_button(out, *button);
        }
        WindowEvent::MouseWheel { delta, phase } => {
            put_u8(out, 13);
            put_wheel(out, *delta);
            put_phase(out, *phase);
        }
        WindowEvent::Touch(t) => {
            put_u8(out, 14);
            put_touch(out, t);
        }
        WindowEvent::RawMouseMotion { dx, dy } => {
            put_u8(out, 15);
            put_i32(out, *dx);
            put_i32(out, *dy);
        }
        WindowEvent::GamepadConnected(GamepadId(id)) => {
            put_u8(out, 16);
            put_u32(out, *id);
        }
        WindowEvent::GamepadDisconnected(GamepadId(id)) => {
            put_u8(out, 17);
            put_u32(out, *id);
        }
        WindowEvent::RedrawRequested => put_u8(out, 18),
        WindowEvent::YieldPoint {
            frame_number,
            timestamp_ns,
        } => {
            put_u8(out, 19);
            put_u64(out, *frame_number);
            put_u64(out, *timestamp_ns);
        }
        // Tags 64 to 72 are the newer variants, numbered apart from the dense range above.
        WindowEvent::ModifiersChanged(mods) => {
            put_u8(out, 64);
            put_u8(out, mods.bits());
        }
        WindowEvent::ThemeChanged(theme) => {
            put_u8(out, 65);
            put_u8(out, matches!(theme, Theme::Dark) as u8);
        }
        WindowEvent::User(value) => {
            put_u8(out, 66);
            put_u64(out, *value);
        }
        WindowEvent::FileHovered { chunk, continued } => {
            put_u8(out, 70);
            put_str(out, chunk.as_str());
            put_bool(out, *continued);
        }
        WindowEvent::FileDropped { chunk, continued } => {
            put_u8(out, 71);
            put_str(out, chunk.as_str());
            put_bool(out, *continued);
        }
        WindowEvent::FileHoverCancelled => put_u8(out, 72),
    }
}

/// Appends a window id and its event, as stored in recordings.
pub fn encode_record(window: WindowId, event: &WindowEvent, out: &mut Vec<u8>) {
    put_u64(out, window.0);
    encode_event(event, out);
}

// ---------------------------------------------------------------------------
// Decoding
// ---------------------------------------------------------------------------

/// A cursor over bytes being decoded.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], CodecError> {
        let end = self.pos.checked_add(n).ok_or(CodecError::Truncated)?;
        let slice = self.buf.get(self.pos..end).ok_or(CodecError::Truncated)?;
        self.pos = end;
        Ok(slice)
    }
    fn array<const N: usize>(&mut self) -> Result<[u8; N], CodecError> {
        let mut out = [0_u8; N];
        out.copy_from_slice(self.take(N)?);
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, CodecError> {
        Ok(self.array::<1>()?[0])
    }
    fn u16(&mut self) -> Result<u16, CodecError> {
        Ok(u16::from_le_bytes(self.array()?))
    }
    fn u32(&mut self) -> Result<u32, CodecError> {
        Ok(u32::from_le_bytes(self.array()?))
    }
    fn i32(&mut self) -> Result<i32, CodecError> {
        Ok(i32::from_le_bytes(self.array()?))
    }
    fn u64(&mut self) -> Result<u64, CodecError> {
        Ok(u64::from_le_bytes(self.array()?))
    }
    fn f32(&mut self) -> Result<f32, CodecError> {
        Ok(f32::from_bits(self.u32()?))
    }
    fn f64(&mut self) -> Result<f64, CodecError> {
        Ok(f64::from_bits(self.u64()?))
    }
    fn bool(&mut self) -> Result<bool, CodecError> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(CodecError::BadValue),
        }
    }
    fn string(&mut self) -> Result<&'a str, CodecError> {
        let len = self.u32()? as usize;
        core::str::from_utf8(self.take(len)?).map_err(|_| CodecError::BadValue)
    }
    /// A chunk of text that must fit one `ImeText`: longer input is not one this encoder wrote.
    fn chunk(&mut self) -> Result<ImeText, CodecError> {
        let text = self.string()?;
        let (chunk, rest) = ImeText::take_chunk(text);
        if rest.is_empty() {
            Ok(chunk)
        } else {
            Err(CodecError::BadValue)
        }
    }
    fn state(&mut self) -> Result<ElementState, CodecError> {
        match self.u8()? {
            0 => Ok(ElementState::Pressed),
            1 => Ok(ElementState::Released),
            _ => Err(CodecError::BadValue),
        }
    }
    fn key(&mut self) -> Result<KeyCode, CodecError> {
        key_from_tag(self.u32()?).ok_or(CodecError::BadValue)
    }
    fn physical(&mut self) -> Result<PhysicalKey, CodecError> {
        match self.u8()? {
            0 => Ok(PhysicalKey::Code(self.key()?)),
            1 => Ok(PhysicalKey::Unidentified(self.u32()?)),
            _ => Err(CodecError::BadValue),
        }
    }
    fn button(&mut self) -> Result<MouseButton, CodecError> {
        Ok(match self.u8()? {
            0 => MouseButton::Left,
            1 => MouseButton::Right,
            2 => MouseButton::Middle,
            3 => MouseButton::Back,
            4 => MouseButton::Forward,
            5 => MouseButton::Other(self.u16()?),
            _ => return Err(CodecError::BadValue),
        })
    }
    fn phase(&mut self) -> Result<TouchPhase, CodecError> {
        Ok(match self.u8()? {
            0 => TouchPhase::Started,
            1 => TouchPhase::Moved,
            2 => TouchPhase::Ended,
            3 => TouchPhase::Cancelled,
            _ => return Err(CodecError::BadValue),
        })
    }
    fn ime(&mut self) -> Result<ImeEvent, CodecError> {
        Ok(match self.u8()? {
            0 => ImeEvent::Enabled,
            1 => ImeEvent::Preedit {
                text: self.chunk()?,
                continued: self.bool()?,
            },
            2 => ImeEvent::Commit {
                text: self.chunk()?,
                continued: self.bool()?,
            },
            3 => ImeEvent::Disabled,
            _ => return Err(CodecError::BadValue),
        })
    }
    fn wheel(&mut self) -> Result<MouseScrollDelta, CodecError> {
        Ok(match self.u8()? {
            0 => MouseScrollDelta::LineDelta {
                x: self.f32()?,
                y: self.f32()?,
            },
            1 => MouseScrollDelta::PixelDelta {
                x: self.f64()?,
                y: self.f64()?,
            },
            _ => return Err(CodecError::BadValue),
        })
    }
    fn touch(&mut self) -> Result<TouchEvent, CodecError> {
        Ok(TouchEvent {
            id: self.u64()?,
            phase: self.phase()?,
            position: PhysicalPosition::new(self.f64()?, self.f64()?),
            force: self.f32()?,
        })
    }
}

/// Decodes one event from the start of `buf`. Returns the event and the number of bytes it used.
pub fn decode_event(buf: &[u8]) -> Result<(WindowEvent, usize), CodecError> {
    let mut r = Reader { buf, pos: 0 };
    let event = read_event(&mut r)?;
    Ok((event, r.pos))
}

/// Decodes a window id and its event, as written by [`encode_record`].
pub fn decode_record(buf: &[u8]) -> Result<(Event, usize), CodecError> {
    let mut r = Reader { buf, pos: 0 };
    let window = WindowId(r.u64()?);
    let event = read_event(&mut r)?;
    Ok((Event { window, event }, r.pos))
}

fn read_event(r: &mut Reader<'_>) -> Result<WindowEvent, CodecError> {
    let tag = r.u8()?;
    Ok(match tag {
        0 => WindowEvent::CloseRequested,
        1 => WindowEvent::Destroyed,
        2 => WindowEvent::Resized(PhysicalSize::new(r.u32()?, r.u32()?)),
        3 => WindowEvent::Moved(PhysicalPosition::new(r.i32()?, r.i32()?)),
        4 => WindowEvent::ScaleFactorChanged {
            scale_factor: r.f64()?,
            new_inner_size: PhysicalSize::new(r.u32()?, r.u32()?),
        },
        5 => WindowEvent::Focused(r.bool()?),
        6 => WindowEvent::Occluded(r.bool()?),
        7 => WindowEvent::KeyboardInput {
            state: r.state()?,
            key_code: r.key()?,
            physical_key: r.physical()?,
        },
        8 => WindowEvent::Ime(r.ime()?),
        9 => WindowEvent::CursorMoved {
            position: PhysicalPosition::new(r.f64()?, r.f64()?),
            pressure: r.f32()?,
        },
        10 => WindowEvent::CursorEntered,
        11 => WindowEvent::CursorLeft,
        12 => WindowEvent::MouseInput {
            state: r.state()?,
            button: r.button()?,
        },
        13 => WindowEvent::MouseWheel {
            delta: r.wheel()?,
            phase: r.phase()?,
        },
        14 => WindowEvent::Touch(r.touch()?),
        15 => WindowEvent::RawMouseMotion {
            dx: r.i32()?,
            dy: r.i32()?,
        },
        16 => WindowEvent::GamepadConnected(GamepadId(r.u32()?)),
        17 => WindowEvent::GamepadDisconnected(GamepadId(r.u32()?)),
        18 => WindowEvent::RedrawRequested,
        19 => WindowEvent::YieldPoint {
            frame_number: r.u64()?,
            timestamp_ns: r.u64()?,
        },
        64 => WindowEvent::ModifiersChanged(ModifiersState::from_bits(r.u8()?)),
        65 => WindowEvent::ThemeChanged(match r.u8()? {
            0 => Theme::Light,
            1 => Theme::Dark,
            _ => return Err(CodecError::BadValue),
        }),
        66 => WindowEvent::User(r.u64()?),
        70 => WindowEvent::FileHovered {
            chunk: r.chunk()?,
            continued: r.bool()?,
        },
        71 => WindowEvent::FileDropped {
            chunk: r.chunk()?,
            continued: r.bool()?,
        },
        72 => WindowEvent::FileHoverCancelled,
        other => return Err(CodecError::BadTag(other)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_numbers_round_trip() {
        for tag in 0..KEYS_BY_TAG.len() as u32 {
            let key = key_from_tag(tag).expect("every number has a key");
            assert_eq!(key_tag(key), tag);
        }
        assert_eq!(key_from_tag(KEYS_BY_TAG.len() as u32), None);
    }

    #[test]
    fn existing_key_numbers_are_unchanged() {
        // These are part of signatures already made; they must not move.
        assert_eq!(key_tag(KeyCode::A), 0);
        assert_eq!(key_tag(KeyCode::Unidentified), 99);
    }
}
