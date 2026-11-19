// SPDX-License-Identifier: MIT OR Apache-2.0
//! Client-side display protocol (versioned wire format).
//!
//! The peer on the other end (the compositor or display server) is still an open decision
//! (see todo.md), so this module defines only the wire format and the version handshake.
//! It does no I/O and has no transport. Shared-memory frame transport needs an OS mapping
//! and is not here.
//!
//! Wire format, version 1. Every message is one frame:
//!
//! ```text
//! u32 LE  length of the rest of the frame (tag + payload), at most MAX_FRAME_LEN
//! u8      tag (see Message)
//! ...     payload: fixed-size little-endian fields in declaration order
//! ```
//!
//! A decoder never allocates from the length alone, so a hostile peer cannot make it reserve
//! memory by sending a large length.

use crate::dpi::PhysicalSize;
use crate::ids::WindowId;
use crate::security::SurfaceHandle;

/// Protocol version. Bumped on any wire-incompatible change.
pub const PROTOCOL_VERSION: u32 = 1;

/// Largest accepted frame body (tag + payload), in bytes. Every message is far smaller.
pub const MAX_FRAME_LEN: u32 = 4096;

const TAG_HELLO: u8 = 1;
const TAG_WELCOME: u8 = 2;
const TAG_SURFACE_READY: u8 = 3;
const TAG_FRAME_PRESENTED: u8 = 4;
const TAG_CLOSE_WINDOW: u8 = 5;

/// A protocol message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Message {
    /// Client opens the session and states the highest version it speaks.
    Hello {
        /// The client's protocol version.
        version: u32,
    },
    /// Server accepts the session at `version`, which is the negotiated version.
    Welcome {
        /// The version both sides will use.
        version: u32,
    },
    /// A window's frame surface is ready to be written to.
    SurfaceReady {
        /// The window.
        window: WindowId,
        /// The surface reference (see [`SurfaceHandle`]).
        surface: SurfaceHandle,
        /// Surface size in physical pixels.
        size: PhysicalSize<u32>,
    },
    /// A frame written to a surface has been presented.
    FramePresented {
        /// The window.
        window: WindowId,
        /// The surface the frame was written to.
        surface: SurfaceHandle,
        /// The frame counter, as in `WindowEvent::YieldPoint`.
        frame: u64,
    },
    /// The client asks the server to close a window.
    CloseWindow {
        /// The window.
        window: WindowId,
    },
}

/// Why bytes could not be decoded into a [`Message`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DecodeError {
    /// The buffer ends before the frame does. More bytes are needed.
    Truncated,
    /// The declared length is larger than [`MAX_FRAME_LEN`].
    TooLong(u32),
    /// The tag is not known to this version.
    UnknownTag(u8),
    /// The payload length does not match the tag's fixed layout.
    BadPayload(u8),
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            DecodeError::Truncated => f.write_str("frame is incomplete"),
            DecodeError::TooLong(n) => write!(f, "frame length {n} exceeds {MAX_FRAME_LEN}"),
            DecodeError::UnknownTag(t) => write!(f, "unknown message tag {t}"),
            DecodeError::BadPayload(t) => write!(f, "payload does not match tag {t}"),
        }
    }
}

impl std::error::Error for DecodeError {}

/// Why two sides cannot agree on a version.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NegotiateError {
    /// The versions differ. Version 1 has no compatible range, so only an exact match works.
    Mismatch {
        /// The client's version.
        client: u32,
        /// The server's version.
        server: u32,
    },
}

/// Chooses the version for a session. Version 1 has no backward-compatible range, so the
/// two sides must match exactly.
pub fn negotiate(client: u32, server: u32) -> Result<u32, NegotiateError> {
    if client == server {
        Ok(client)
    } else {
        Err(NegotiateError::Mismatch { client, server })
    }
}

/// Appends the wire encoding of `message` to `out`.
pub fn encode(message: &Message, out: &mut Vec<u8>) {
    let mut body = Vec::with_capacity(32);
    match *message {
        Message::Hello { version } => {
            body.push(TAG_HELLO);
            body.extend_from_slice(&version.to_le_bytes());
        }
        Message::Welcome { version } => {
            body.push(TAG_WELCOME);
            body.extend_from_slice(&version.to_le_bytes());
        }
        Message::SurfaceReady {
            window,
            surface,
            size,
        } => {
            body.push(TAG_SURFACE_READY);
            body.extend_from_slice(&window.0.to_le_bytes());
            body.extend_from_slice(&surface.id.to_le_bytes());
            body.extend_from_slice(&size.width.to_le_bytes());
            body.extend_from_slice(&size.height.to_le_bytes());
        }
        Message::FramePresented {
            window,
            surface,
            frame,
        } => {
            body.push(TAG_FRAME_PRESENTED);
            body.extend_from_slice(&window.0.to_le_bytes());
            body.extend_from_slice(&surface.id.to_le_bytes());
            body.extend_from_slice(&frame.to_le_bytes());
        }
        Message::CloseWindow { window } => {
            body.push(TAG_CLOSE_WINDOW);
            body.extend_from_slice(&window.0.to_le_bytes());
        }
    }
    out.extend_from_slice(&(body.len() as u32).to_le_bytes());
    out.extend_from_slice(&body);
}

/// Decodes one frame from the start of `buf`. Returns the message and the number of bytes it
/// used, so the caller can decode the next frame from the rest of the buffer.
pub fn decode(buf: &[u8]) -> Result<(Message, usize), DecodeError> {
    let len_bytes: [u8; 4] = buf
        .get(..4)
        .and_then(|b| b.try_into().ok())
        .ok_or(DecodeError::Truncated)?;
    let len = u32::from_le_bytes(len_bytes);
    if len > MAX_FRAME_LEN {
        return Err(DecodeError::TooLong(len));
    }
    if len == 0 {
        return Err(DecodeError::BadPayload(0));
    }
    let end = 4 + len as usize;
    let body = buf.get(4..end).ok_or(DecodeError::Truncated)?;
    let tag = body[0];
    let payload = &body[1..];

    let u32_at = |i: usize| -> u32 { u32::from_le_bytes(payload[i..i + 4].try_into().unwrap()) };
    let u64_at = |i: usize| -> u64 { u64::from_le_bytes(payload[i..i + 8].try_into().unwrap()) };
    let expect = |n: usize| -> Result<(), DecodeError> {
        if payload.len() == n {
            Ok(())
        } else {
            Err(DecodeError::BadPayload(tag))
        }
    };

    let message = match tag {
        TAG_HELLO => {
            expect(4)?;
            Message::Hello { version: u32_at(0) }
        }
        TAG_WELCOME => {
            expect(4)?;
            Message::Welcome { version: u32_at(0) }
        }
        TAG_SURFACE_READY => {
            expect(8 + 8 + 4 + 4)?;
            Message::SurfaceReady {
                window: WindowId(u64_at(0)),
                surface: SurfaceHandle { id: u64_at(8) },
                size: PhysicalSize::new(u32_at(16), u32_at(20)),
            }
        }
        TAG_FRAME_PRESENTED => {
            expect(8 + 8 + 8)?;
            Message::FramePresented {
                window: WindowId(u64_at(0)),
                surface: SurfaceHandle { id: u64_at(8) },
                frame: u64_at(16),
            }
        }
        TAG_CLOSE_WINDOW => {
            expect(8)?;
            Message::CloseWindow {
                window: WindowId(u64_at(0)),
            }
        }
        other => return Err(DecodeError::UnknownTag(other)),
    };
    Ok((message, end))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn samples() -> Vec<Message> {
        vec![
            Message::Hello { version: 1 },
            Message::Welcome { version: 1 },
            Message::SurfaceReady {
                window: WindowId(7),
                surface: SurfaceHandle { id: 99 },
                size: PhysicalSize::new(1920, 1080),
            },
            Message::FramePresented {
                window: WindowId(7),
                surface: SurfaceHandle { id: 99 },
                frame: 123_456,
            },
            Message::CloseWindow {
                window: WindowId(7),
            },
        ]
    }

    #[test]
    fn every_message_round_trips() {
        for m in samples() {
            let mut buf = Vec::new();
            encode(&m, &mut buf);
            let (decoded, used) = decode(&buf).unwrap();
            assert_eq!(decoded, m);
            assert_eq!(used, buf.len());
        }
    }

    #[test]
    fn frames_decode_back_to_back() {
        let mut buf = Vec::new();
        for m in samples() {
            encode(&m, &mut buf);
        }
        let mut rest = &buf[..];
        let mut got = Vec::new();
        while !rest.is_empty() {
            let (m, used) = decode(rest).unwrap();
            got.push(m);
            rest = &rest[used..];
        }
        assert_eq!(got, samples());
    }

    #[test]
    fn a_partial_frame_is_truncated_not_an_error_of_another_kind() {
        let mut buf = Vec::new();
        encode(&Message::Hello { version: 1 }, &mut buf);
        for cut in 0..buf.len() {
            assert_eq!(
                decode(&buf[..cut]),
                Err(DecodeError::Truncated),
                "cut {cut}"
            );
        }
    }

    #[test]
    fn oversized_length_is_refused_before_any_allocation_of_that_size() {
        let buf = (MAX_FRAME_LEN + 1).to_le_bytes();
        assert_eq!(decode(&buf), Err(DecodeError::TooLong(MAX_FRAME_LEN + 1)));
        let huge = u32::MAX.to_le_bytes();
        assert_eq!(decode(&huge), Err(DecodeError::TooLong(u32::MAX)));
    }

    #[test]
    fn unknown_tags_and_wrong_payload_sizes_are_errors() {
        // Length 1, tag 200: well-formed frame with a tag this version does not know.
        assert_eq!(
            decode(&[1, 0, 0, 0, 200]),
            Err(DecodeError::UnknownTag(200))
        );
        // Hello with a 3-byte payload instead of 4.
        assert_eq!(
            decode(&[4, 0, 0, 0, TAG_HELLO, 1, 2, 3]),
            Err(DecodeError::BadPayload(TAG_HELLO))
        );
        // Zero-length frame has no tag.
        assert_eq!(decode(&[0, 0, 0, 0]), Err(DecodeError::BadPayload(0)));
    }

    #[test]
    fn version_negotiation_requires_an_exact_match() {
        assert_eq!(negotiate(1, 1), Ok(1));
        assert_eq!(
            negotiate(1, 2),
            Err(NegotiateError::Mismatch {
                client: 1,
                server: 2
            })
        );
        assert_eq!(PROTOCOL_VERSION, 1);
    }
}
