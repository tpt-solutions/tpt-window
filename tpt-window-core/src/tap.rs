// SPDX-License-Identifier: MIT OR Apache-2.0
//! A tap on the display protocol: a passive record of the frames that cross a [`FrameRing`],
//! for inspecting a session without changing it.
//!
//! Give the tap a copy of each message as it is pushed, or decode a byte dump of a ring with
//! [`describe_stream`]. The tap never touches the ring, so it cannot change what the peers see.
//!
//! ```
//! use tpt_window_core::protocol::Message;
//! use tpt_window_core::tap::Tap;
//!
//! let mut tap = Tap::new();
//! tap.observe(&Message::Hello { version: 1 });
//! assert_eq!(tap.entries().len(), 1);
//! assert!(tap.describe().contains("Hello"));
//! ```
//!
//! [`FrameRing`]: crate::channel::FrameRing

use crate::protocol::{self, DecodeError, Message};

/// One message seen by the tap.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TapEntry {
    /// Position in the order the tap saw messages, starting at zero.
    pub index: u64,
    /// The message.
    pub message: Message,
    /// Its size on the wire, length prefix included.
    pub wire_bytes: usize,
}

/// Records messages in order and describes them.
#[derive(Clone, Debug, Default)]
pub struct Tap {
    entries: Vec<TapEntry>,
}

impl Tap {
    /// An empty tap.
    pub fn new() -> Self {
        Self::default()
    }

    /// Records one message. Call it with each message pushed to a ring.
    pub fn observe(&mut self, message: &Message) {
        let mut bytes = Vec::with_capacity(32);
        protocol::encode(message, &mut bytes);
        self.entries.push(TapEntry {
            index: self.entries.len() as u64,
            message: *message,
            wire_bytes: bytes.len(),
        });
    }

    /// The messages seen so far, oldest first.
    pub fn entries(&self) -> &[TapEntry] {
        &self.entries
    }

    /// A line per message: its index, its size and its contents.
    pub fn describe(&self) -> String {
        let mut out = String::new();
        for entry in &self.entries {
            out.push_str(&format!(
                "{:>6}  {:>4} B  {}\n",
                entry.index,
                entry.wire_bytes,
                describe_message(&entry.message)
            ));
        }
        out
    }
}

/// One line describing a message, in a form meant for people reading a log.
pub fn describe_message(message: &Message) -> String {
    format!("{message:?}")
}

/// Decodes back-to-back frames from a byte dump and describes each one. Stops at the first
/// frame that does not decode, and returns that error with the lines decoded so far.
pub fn describe_stream(bytes: &[u8]) -> (Vec<String>, Option<DecodeError>) {
    let mut lines = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        match protocol::decode(&bytes[pos..]) {
            Ok((message, used)) => {
                lines.push(format!(
                    "{pos:>6}  {used:>4} B  {}",
                    describe_message(&message)
                ));
                pos += used;
            }
            Err(e) => return (lines, Some(e)),
        }
    }
    (lines, None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::encode;

    #[test]
    fn the_tap_records_wire_sizes_in_order() {
        let mut tap = Tap::new();
        tap.observe(&Message::Hello { version: 1 });
        tap.observe(&Message::Welcome { version: 1 });
        let entries = tap.entries();
        assert_eq!(entries[0].index, 0);
        assert_eq!(entries[1].index, 1);
        let mut check = Vec::new();
        encode(&Message::Hello { version: 1 }, &mut check);
        assert_eq!(entries[0].wire_bytes, check.len());
    }

    #[test]
    fn a_dump_decodes_frame_by_frame_and_reports_damage() {
        let mut dump = Vec::new();
        encode(&Message::Hello { version: 1 }, &mut dump);
        encode(&Message::Welcome { version: 1 }, &mut dump);
        let (lines, err) = describe_stream(&dump);
        assert_eq!(lines.len(), 2);
        assert!(err.is_none());
        assert!(lines[1].contains("Welcome"));

        dump.push(0xFF);
        let (lines, err) = describe_stream(&dump);
        assert_eq!(lines.len(), 2);
        assert!(err.is_some(), "the stray byte is reported");
    }
}
