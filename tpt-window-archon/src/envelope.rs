// SPDX-License-Identifier: MIT OR Apache-2.0
//! Signed input events.
//!
//! The trusted router signs every event it delivers with an Ed25519 key. The application
//! verifies with the matching public key, so it can check origin but cannot create
//! events. Each signature covers the window id, a per-window sequence number and a
//! canonical encoding of the event. Changing any of them breaks the signature.
//!
//! The sequence number must strictly increase per window. A replayed or reordered event
//! is rejected even though its signature is valid.
//!
//! The encoding is written out by hand, so it does not depend on `Debug` or on memory
//! layout, and a new [`WindowEvent`] variant fails to compile until it is encoded here.

use std::collections::BTreeMap;

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use tpt_window_core::{
    ElementState, GamepadId, ImeEvent, KeyCode, MouseButton, MouseScrollDelta, PhysicalKey,
    PhysicalPosition, PhysicalSize, TouchEvent, TouchPhase, WindowEvent, WindowId,
};

/// Domain separator, so a signature over these bytes cannot be reused for another purpose.
const DOMAIN: &[u8] = b"tpt-window/input-event/v1";

/// An event with the signature the router attached to it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignedEvent {
    /// The window the event is for.
    pub window: WindowId,
    /// Sequence number, starting at 1 and increasing per window.
    pub seq: u64,
    /// The event.
    pub event: WindowEvent,
    /// Ed25519 signature over the canonical bytes.
    pub signature: [u8; 64],
}

/// Why a [`SignedEvent`] was rejected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerifyError {
    /// The signature does not match the event, or it was made by another key.
    BadSignature,
    /// The sequence number is not newer than the last one accepted for this window.
    Replayed {
        /// The window.
        window: WindowId,
        /// The rejected sequence number.
        seq: u64,
        /// The last sequence number accepted for the window.
        last: u64,
    },
}

impl core::fmt::Display for VerifyError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            VerifyError::BadSignature => f.write_str("event signature is invalid"),
            VerifyError::Replayed { window, seq, last } => write!(
                f,
                "event {seq} for window {} is not newer than {last}",
                window.0
            ),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Signs events on the trusted side (the router).
pub struct EventSigner {
    key: SigningKey,
    last_seq: BTreeMap<WindowId, u64>,
}

impl EventSigner {
    /// Creates a signer. Key material is the caller's responsibility (for example, a key
    /// that archon keeps in its own protected store).
    pub fn new(key: SigningKey) -> Self {
        Self {
            key,
            last_seq: BTreeMap::new(),
        }
    }

    /// The public key that receivers verify with.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.key.verifying_key()
    }

    /// Signs `event` for `window`, assigning the next sequence number for that window.
    pub fn sign(&mut self, window: WindowId, event: WindowEvent) -> SignedEvent {
        let seq = self.last_seq.entry(window).or_insert(0);
        *seq += 1;
        let seq = *seq;
        let message = signing_bytes(window, seq, &event);
        let signature = self.key.sign(&message).to_bytes();
        SignedEvent {
            window,
            seq,
            event,
            signature,
        }
    }
}

/// Verifies events on the application side. Holds only the public key.
pub struct EventVerifier {
    key: VerifyingKey,
    last_seq: BTreeMap<WindowId, u64>,
}

impl EventVerifier {
    /// Creates a verifier for events signed by the router holding the matching secret key.
    pub fn new(key: VerifyingKey) -> Self {
        Self {
            key,
            last_seq: BTreeMap::new(),
        }
    }

    /// Checks the signature, then the sequence number. Returns the event if both pass.
    ///
    /// The signature is checked first, so a forged event cannot move the sequence state.
    pub fn verify(&mut self, signed: &SignedEvent) -> Result<WindowEvent, VerifyError> {
        let message = signing_bytes(signed.window, signed.seq, &signed.event);
        let signature = Signature::from_bytes(&signed.signature);
        // `verify_strict` also rejects non-canonical signatures and small-order keys, so one
        // signed event has exactly one accepted encoding.
        self.key
            .verify_strict(&message, &signature)
            .map_err(|_| VerifyError::BadSignature)?;

        let last = self.last_seq.get(&signed.window).copied().unwrap_or(0);
        if signed.seq <= last {
            return Err(VerifyError::Replayed {
                window: signed.window,
                seq: signed.seq,
                last,
            });
        }
        self.last_seq.insert(signed.window, signed.seq);
        Ok(signed.event)
    }
}

/// The exact bytes that are signed: domain, window id, sequence number, event encoding.
fn signing_bytes(window: WindowId, seq: u64, event: &WindowEvent) -> Vec<u8> {
    let mut out = Vec::with_capacity(96);
    out.extend_from_slice(DOMAIN);
    out.extend_from_slice(&window.0.to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
    encode_event(event, &mut out);
    out
}

// ---------------------------------------------------------------------------
// Canonical encoding. Every variant has an explicit tag. Floats are encoded by bit
// pattern, so -0.0 and 0.0 differ and NaN payloads are preserved.
// ---------------------------------------------------------------------------

fn put_u8(out: &mut Vec<u8>, v: u8) {
    out.push(v);
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
            out.extend_from_slice(&n.to_le_bytes());
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
/// Tag for a key this crate does not know yet. `KeyCode` is `#[non_exhaustive]`, so a variant
/// added in core must get a row in [`key_tag`]. Until it does, every such key shares this tag
/// and cannot be told apart in the signed bytes. The test `key_tags_are_unique` guards the
/// known rows.
const UNKNOWN_KEY_TAG: u32 = u32::MAX;

/// Stable wire tag for a key. These numbers are part of the signed bytes, so they are written
/// out here and never derived from the enum's declaration order. Reordering the variants of
/// `KeyCode` therefore cannot change a signature.
fn key_tag(k: KeyCode) -> u32 {
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
        _ => UNKNOWN_KEY_TAG,
    }
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
///
/// This match has no wildcard arm, so a new `WindowEvent` variant is a compile error here.
fn encode_event(event: &WindowEvent, out: &mut Vec<u8>) {
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
        WindowEvent::ModifiersChanged(mods) => {
            // Tag 64, away from the dense range the other variants use.
            put_u8(out, 64);
            put_u8(out, mods.bits());
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_window_core::{ImeText, KeyCode, PhysicalKey};

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn sample_events() -> Vec<WindowEvent> {
        vec![
            WindowEvent::CloseRequested,
            WindowEvent::Resized(PhysicalSize::new(800, 600)),
            WindowEvent::KeyboardInput {
                state: ElementState::Pressed,
                key_code: KeyCode::A,
                physical_key: PhysicalKey::Code(KeyCode::A),
            },
            WindowEvent::Ime(ImeEvent::Commit {
                text: ImeText::new("日本"),
                continued: false,
            }),
            WindowEvent::CursorMoved {
                position: PhysicalPosition::new(10.5, 20.25),
                pressure: 0.5,
            },
            WindowEvent::MouseInput {
                state: ElementState::Released,
                button: MouseButton::Other(9),
            },
            WindowEvent::MouseWheel {
                delta: MouseScrollDelta::PixelDelta { x: 1.0, y: -2.0 },
                phase: TouchPhase::Moved,
            },
            WindowEvent::RawMouseMotion { dx: -3, dy: 4 },
            WindowEvent::YieldPoint {
                frame_number: 9,
                timestamp_ns: 1_000,
            },
        ]
    }

    #[test]
    fn signed_events_verify_in_order() {
        let mut signer = EventSigner::new(key());
        let mut verifier = EventVerifier::new(signer.verifying_key());
        for event in sample_events() {
            let signed = signer.sign(WindowId(3), event);
            assert_eq!(verifier.verify(&signed), Ok(event));
        }
    }

    #[test]
    fn altered_event_fails_the_signature() {
        let mut signer = EventSigner::new(key());
        let mut verifier = EventVerifier::new(signer.verifying_key());
        let mut signed = signer.sign(
            WindowId(1),
            WindowEvent::KeyboardInput {
                state: ElementState::Released,
                key_code: KeyCode::Escape,
                physical_key: PhysicalKey::Code(KeyCode::Escape),
            },
        );
        // A local process rewrites the key it is about to hand to the app.
        signed.event = WindowEvent::KeyboardInput {
            state: ElementState::Released,
            key_code: KeyCode::Enter,
            physical_key: PhysicalKey::Code(KeyCode::Enter),
        };
        assert_eq!(verifier.verify(&signed), Err(VerifyError::BadSignature));
    }

    #[test]
    fn altered_window_or_sequence_fails_the_signature() {
        let mut signer = EventSigner::new(key());
        let mut verifier = EventVerifier::new(signer.verifying_key());
        let signed = signer.sign(WindowId(1), WindowEvent::Focused(true));

        let mut other_window = signed;
        other_window.window = WindowId(2);
        assert_eq!(
            verifier.verify(&other_window),
            Err(VerifyError::BadSignature)
        );

        let mut other_seq = signed;
        other_seq.seq = 99;
        assert_eq!(verifier.verify(&other_seq), Err(VerifyError::BadSignature));
    }

    #[test]
    fn event_signed_by_another_key_is_rejected() {
        let mut attacker = EventSigner::new(SigningKey::from_bytes(&[9u8; 32]));
        let signer = EventSigner::new(key());
        let mut verifier = EventVerifier::new(signer.verifying_key());
        let forged = attacker.sign(WindowId(1), WindowEvent::Focused(true));
        assert_eq!(verifier.verify(&forged), Err(VerifyError::BadSignature));
    }

    #[test]
    fn replayed_and_reordered_events_are_rejected() {
        let mut signer = EventSigner::new(key());
        let mut verifier = EventVerifier::new(signer.verifying_key());
        let first = signer.sign(WindowId(1), WindowEvent::Focused(true));
        let second = signer.sign(WindowId(1), WindowEvent::Focused(false));

        assert!(verifier.verify(&second).is_ok());
        // The first event is genuine, but it is older than what was already accepted.
        assert_eq!(
            verifier.verify(&first),
            Err(VerifyError::Replayed {
                window: WindowId(1),
                seq: 1,
                last: 2
            })
        );
        // Replaying the same event again is rejected too.
        assert!(matches!(
            verifier.verify(&second),
            Err(VerifyError::Replayed { .. })
        ));
    }

    #[test]
    fn sequence_numbers_are_per_window() {
        let mut signer = EventSigner::new(key());
        let mut verifier = EventVerifier::new(signer.verifying_key());
        let a = signer.sign(WindowId(1), WindowEvent::Focused(true));
        let b = signer.sign(WindowId(2), WindowEvent::Focused(true));
        assert_eq!((a.seq, b.seq), (1, 1));
        assert!(verifier.verify(&a).is_ok());
        assert!(verifier.verify(&b).is_ok());
    }

    #[test]
    fn encoding_separates_look_alike_events() {
        let encode = |e: &WindowEvent| {
            let mut out = Vec::new();
            encode_event(e, &mut out);
            out
        };
        let pairs = [
            (
                WindowEvent::Resized(PhysicalSize::new(1, 2)),
                WindowEvent::Resized(PhysicalSize::new(2, 1)),
            ),
            (WindowEvent::Focused(true), WindowEvent::Occluded(true)),
            (
                WindowEvent::CursorMoved {
                    position: PhysicalPosition::new(0.0, 0.0),
                    pressure: 0.0,
                },
                WindowEvent::CursorMoved {
                    position: PhysicalPosition::new(-0.0, 0.0),
                    pressure: 0.0,
                },
            ),
            (
                WindowEvent::KeyboardInput {
                    state: ElementState::Pressed,
                    key_code: KeyCode::A,
                    physical_key: PhysicalKey::Code(KeyCode::A),
                },
                WindowEvent::KeyboardInput {
                    state: ElementState::Pressed,
                    key_code: KeyCode::A,
                    physical_key: PhysicalKey::Unidentified(KeyCode::A as u32),
                },
            ),
            (
                WindowEvent::Ime(ImeEvent::Commit {
                    text: ImeText::new("x"),
                    continued: false,
                }),
                WindowEvent::Ime(ImeEvent::Preedit {
                    text: ImeText::new("x"),
                    continued: false,
                }),
            ),
            (
                WindowEvent::MouseWheel {
                    delta: MouseScrollDelta::LineDelta { x: 1.0, y: 1.0 },
                    phase: TouchPhase::Moved,
                },
                WindowEvent::MouseWheel {
                    delta: MouseScrollDelta::PixelDelta { x: 1.0, y: 1.0 },
                    phase: TouchPhase::Moved,
                },
            ),
        ];
        for (a, b) in pairs {
            assert_ne!(encode(&a), encode(&b), "{a:?} vs {b:?}");
        }
    }
}

#[cfg(test)]
mod key_tag_tests {
    use super::*;

    #[test]
    fn key_tags_are_stable_for_known_keys() {
        // These values are part of existing signatures. Changing one breaks verification.
        assert_eq!(key_tag(KeyCode::A), 0);
        assert_eq!(key_tag(KeyCode::B), 1);
        assert_eq!(key_tag(KeyCode::NumpadDecimal), 98);
        assert_eq!(key_tag(KeyCode::Unidentified), 99);
        assert_ne!(key_tag(KeyCode::A), key_tag(KeyCode::B));
        // Keys added after the first release get rows of their own, not the unknown tag.
        assert_eq!(key_tag(KeyCode::NumLock), 100);
        assert_eq!(key_tag(KeyCode::F24), 100 + 16);
        assert_ne!(key_tag(KeyCode::MediaStop), UNKNOWN_KEY_TAG);
    }
}
