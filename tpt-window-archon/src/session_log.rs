// SPDX-License-Identifier: MIT OR Apache-2.0
//! Signed, tamper-evident session logs.
//!
//! A session log is the recorded event stream (see `tpt_window_core::record`) with an Ed25519
//! signature on every record. Each signature covers the record's sequence number, the signature
//! of the record before it, and the record's bytes. So the log is a chain: changing a record,
//! dropping one, inserting one, or reordering them all break verification, and the first record
//! that fails is named.
//!
//! The signing key stays with the recording side. Anyone with the public key can check a log
//! but cannot make one. Verification is strict, as for input events.
//!
//! ```
//! use ed25519_dalek::SigningKey;
//! use tpt_window_archon::session_log::{SessionLogSigner, verify_session_log};
//! use tpt_window_core::{Event, WindowEvent, WindowId};
//!
//! let key = SigningKey::from_bytes(&[7; 32]);
//! let mut signer = SessionLogSigner::new(key.clone());
//! let mut log = signer.header();
//! signer.append(&Event { window: WindowId(1), event: WindowEvent::Focused(true) }, &mut log);
//! signer.finish(&mut log); // without the end record, the log is reported as unfinished
//!
//! let events = verify_session_log(&key.verifying_key(), &log).unwrap();
//! assert_eq!(events[0].event, WindowEvent::Focused(true));
//! ```

use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use tpt_window_core::Event;
use tpt_window_core::codec::{CodecError, decode_record, encode_record};

/// Domain separator for session-log signatures. A signature made for input events, or for any
/// other purpose, cannot be replayed as a log record.
const DOMAIN: &[u8] = b"tpt-window/session-log/v1";

/// The first eight bytes of a signed log. The last two digits are the format version.
pub const MAGIC: &[u8; 8] = b"TPTSLG01";

/// Length of an Ed25519 signature.
const SIG_LEN: usize = 64;

/// Signature of the record before the first one: 64 zero bytes.
const GENESIS: [u8; SIG_LEN] = [0; SIG_LEN];

/// Why a signed log could not be verified.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogError {
    /// The log does not start with [`MAGIC`].
    BadMagic,
    /// The log ends inside record `record`, which is zero-based.
    Truncated {
        /// Zero-based index of the damaged record.
        record: usize,
    },
    /// Record `record` has sequence number `found` where `expected` was due: a record was
    /// dropped, repeated or inserted.
    Sequence {
        /// Zero-based index of the record.
        record: usize,
        /// The sequence number the record should carry.
        expected: u64,
        /// The sequence number it carries.
        found: u64,
    },
    /// The signature on record `record` does not verify: the record, its position, or the record
    /// before it has changed.
    BadSignature {
        /// Zero-based index of the record.
        record: usize,
    },
    /// The log has no end record. It was cut short, or never finished.
    Unfinished,
    /// The end record is not the last thing in the log.
    TrailingData {
        /// Zero-based index of the end record.
        record: usize,
    },
    /// The record's event bytes do not decode.
    Codec {
        /// Zero-based index of the record.
        record: usize,
        /// What was wrong with the bytes.
        error: CodecError,
    },
}

impl core::fmt::Display for LogError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            LogError::BadMagic => f.write_str("not a signed tpt-window session log"),
            LogError::Truncated { record } => write!(f, "session log ends inside record {record}"),
            LogError::Sequence {
                record,
                expected,
                found,
            } => write!(
                f,
                "record {record} carries sequence {found}, expected {expected}"
            ),
            LogError::BadSignature { record } => {
                write!(f, "record {record} does not verify: the log was changed")
            }
            LogError::Unfinished => {
                f.write_str("session log has no end record: it was cut short or not finished")
            }
            LogError::TrailingData { record } => {
                write!(f, "bytes follow the end record {record}")
            }
            LogError::Codec { record, error } => write!(f, "record {record}: {error}"),
        }
    }
}

impl std::error::Error for LogError {}

/// The bytes a record's signature covers.
fn signed_bytes(seq: u64, prev: &[u8; SIG_LEN], payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(DOMAIN.len() + 8 + SIG_LEN + payload.len());
    out.extend_from_slice(DOMAIN);
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(prev);
    out.extend_from_slice(payload);
    out
}

/// Writes a signed log as events happen. Keep one signer per log: it holds the chain.
pub struct SessionLogSigner {
    key: SigningKey,
    next_seq: u64,
    prev: [u8; SIG_LEN],
}

impl SessionLogSigner {
    /// A signer for a new log. The signing key stays with the recording side.
    pub fn new(key: SigningKey) -> Self {
        Self {
            key,
            next_seq: 0,
            prev: GENESIS,
        }
    }

    /// The magic header. Write it first, once, to the log.
    pub fn header(&self) -> Vec<u8> {
        MAGIC.to_vec()
    }

    /// Appends one signed record for `event` to `out`.
    pub fn append(&mut self, event: &Event, out: &mut Vec<u8>) {
        let mut payload = Vec::with_capacity(48);
        encode_record(event.window, &event.event, &mut payload);
        self.push_raw(&payload, out);
    }

    /// Appends the signed end record. A log without it is reported as unfinished, so a log cut
    /// short at a record boundary is not mistaken for a complete one.
    pub fn finish(&mut self, out: &mut Vec<u8>) {
        self.push_raw(&[], out);
    }

    fn push_raw(&mut self, payload: &[u8], out: &mut Vec<u8>) {
        let seq = self.next_seq;
        let signature = self
            .key
            .sign(&signed_bytes(seq, &self.prev, payload))
            .to_bytes();
        out.extend_from_slice(&seq.to_le_bytes());
        out.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        out.extend_from_slice(payload);
        out.extend_from_slice(&signature);
        self.prev = signature;
        self.next_seq += 1;
    }
}

/// Checks a signed log against `key` and returns its events, in order. Fails at the first record
/// that does not check out.
pub fn verify_session_log(key: &VerifyingKey, log: &[u8]) -> Result<Vec<Event>, LogError> {
    if log.get(..MAGIC.len()) != Some(MAGIC.as_slice()) {
        return Err(LogError::BadMagic);
    }
    let mut pos = MAGIC.len();
    let mut prev = GENESIS;
    let mut events = Vec::new();
    let mut index = 0_usize;
    let mut finished = false;
    while pos < log.len() {
        let truncated = LogError::Truncated { record: index };
        let header = log.get(pos..pos + 12).ok_or(truncated.clone())?;
        let seq = u64::from_le_bytes(header[..8].try_into().map_err(|_| truncated.clone())?);
        let len =
            u32::from_le_bytes(header[8..12].try_into().map_err(|_| truncated.clone())?) as usize;
        let payload_start = pos + 12;
        let payload_end = payload_start + len;
        let payload = log
            .get(payload_start..payload_end)
            .ok_or(truncated.clone())?;
        let sig_bytes: [u8; SIG_LEN] = log
            .get(payload_end..payload_end + SIG_LEN)
            .ok_or(truncated)?
            .try_into()
            .map_err(|_| LogError::Truncated { record: index })?;

        if seq != index as u64 {
            return Err(LogError::Sequence {
                record: index,
                expected: index as u64,
                found: seq,
            });
        }
        let signature = Signature::from_bytes(&sig_bytes);
        key.verify_strict(&signed_bytes(seq, &prev, payload), &signature)
            .map_err(|_| LogError::BadSignature { record: index })?;

        pos = payload_end + SIG_LEN;
        if len == 0 {
            // The end record carries no event, and nothing may follow it.
            if pos != log.len() {
                return Err(LogError::TrailingData { record: index });
            }
            finished = true;
            break;
        }
        let (event, used) = decode_record(payload).map_err(|error| LogError::Codec {
            record: index,
            error,
        })?;
        if used != payload.len() {
            return Err(LogError::Codec {
                record: index,
                error: CodecError::BadValue,
            });
        }
        events.push(event);
        prev = sig_bytes;
        index += 1;
    }
    if !finished {
        return Err(LogError::Unfinished);
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_window_core::{WindowEvent, WindowId};

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[42; 32])
    }

    fn sample_log(n: u64) -> Vec<u8> {
        let mut signer = SessionLogSigner::new(key());
        let mut log = signer.header();
        for i in 0..n {
            signer.append(
                &Event {
                    window: WindowId(1),
                    event: WindowEvent::User(i),
                },
                &mut log,
            );
        }
        signer.finish(&mut log);
        log
    }

    #[test]
    fn a_signed_log_verifies_and_returns_its_events() {
        let log = sample_log(4);
        let events = verify_session_log(&key().verifying_key(), &log).unwrap();
        let values: Vec<u64> = events
            .iter()
            .map(|e| match e.event {
                WindowEvent::User(v) => v,
                _ => panic!("unexpected event"),
            })
            .collect();
        assert_eq!(values, vec![0, 1, 2, 3]);
    }

    #[test]
    fn a_changed_byte_is_caught_at_its_record() {
        let mut log = sample_log(3);
        // Flip the last byte of the second record's payload (its user value).
        let first_len = u32::from_le_bytes(log[8 + 8..8 + 12].try_into().unwrap()) as usize;
        let second_payload_end = 8 + 12 + first_len + SIG_LEN + 12 + first_len;
        log[second_payload_end - 1] ^= 1;
        assert_eq!(
            verify_session_log(&key().verifying_key(), &log),
            Err(LogError::BadSignature { record: 1 })
        );
    }

    #[test]
    fn a_wrong_key_fails_at_the_first_record() {
        let log = sample_log(2);
        let other = SigningKey::from_bytes(&[1; 32]).verifying_key();
        assert_eq!(
            verify_session_log(&other, &log),
            Err(LogError::BadSignature { record: 0 })
        );
    }

    #[test]
    fn a_log_cut_at_a_record_boundary_is_unfinished() {
        let mut signer = SessionLogSigner::new(key());
        let mut log = signer.header();
        signer.append(
            &Event {
                window: WindowId(1),
                event: WindowEvent::User(0),
            },
            &mut log,
        );
        // No end record: a complete-looking log that is really a prefix.
        assert_eq!(
            verify_session_log(&key().verifying_key(), &log),
            Err(LogError::Unfinished)
        );
    }

    #[test]
    fn bytes_after_the_end_record_are_rejected() {
        let mut log = sample_log(1);
        log.push(0);
        assert!(matches!(
            verify_session_log(&key().verifying_key(), &log),
            Err(LogError::TrailingData { .. })
        ));
    }

    #[test]
    fn an_empty_session_is_finished_and_has_no_events() {
        let mut signer = SessionLogSigner::new(key());
        let mut log = signer.header();
        signer.finish(&mut log);
        assert_eq!(
            verify_session_log(&key().verifying_key(), &log).unwrap(),
            Vec::new()
        );
    }

    #[test]
    fn a_dropped_record_is_caught_by_its_sequence_number() {
        let full = sample_log(3);
        // Record 0 occupies the bytes after the header; cut it out.
        let first_len = u32::from_le_bytes(full[8 + 8..8 + 12].try_into().unwrap()) as usize;
        let first_end = 8 + 12 + first_len + SIG_LEN;
        let mut log = full[..8].to_vec();
        log.extend_from_slice(&full[first_end..]);
        assert_eq!(
            verify_session_log(&key().verifying_key(), &log),
            Err(LogError::Sequence {
                record: 0,
                expected: 0,
                found: 1
            })
        );
    }

    #[test]
    fn a_truncated_log_is_reported_not_panicked() {
        let log = sample_log(2);
        for cut in 0..log.len() {
            let result = verify_session_log(&key().verifying_key(), &log[..cut]);
            assert!(result.is_err(), "cut at {cut} must fail");
        }
        assert_eq!(
            verify_session_log(&key().verifying_key(), b"nope"),
            Err(LogError::BadMagic)
        );
    }
}
