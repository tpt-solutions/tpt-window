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
use tpt_window_core::WindowEvent;
use tpt_window_core::WindowId;
use tpt_window_core::codec::encode_event;
#[cfg(test)]
use tpt_window_core::{
    ElementState, ImeEvent, MouseButton, MouseScrollDelta, PhysicalPosition, PhysicalSize,
    TouchPhase,
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
    use tpt_window_core::KeyCode;
    use tpt_window_core::codec::key_tag;

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
        assert_ne!(key_tag(KeyCode::MediaStop), u32::MAX);
    }
}
