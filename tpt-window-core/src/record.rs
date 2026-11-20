// SPDX-License-Identifier: MIT OR Apache-2.0
//! Recording a window event stream, and reading the recording back.
//!
//! A log is a magic header followed by records. Each record is a little-endian `u32` length and
//! the window id and event in the [`codec`](crate::codec) encoding. Records keep their order, so
//! a log is the exact sequence the loop delivered, yield points included.
//!
//! Record after each poll, so the log has the frame boundaries:
//!
//! ```
//! use tpt_window_core::*;
//! use tpt_window_core::record::{Recorder, read_log};
//!
//! let mut queue = EventQueue::with_capacity(8);
//! let mut recorder = Recorder::new();
//! queue.push(Event { window: WindowId(1), event: WindowEvent::Focused(true) });
//! recorder.record_queue(&mut queue);
//! let events = read_log(recorder.as_bytes()).unwrap();
//! assert_eq!(events[0].event, WindowEvent::Focused(true));
//! ```

use std::fmt;

use crate::codec::{CodecError, decode_record, encode_record};
use crate::event::Event;
use crate::sink::EventQueue;

/// The first eight bytes of every log. The last two digits are the format version.
pub const MAGIC: &[u8; 8] = b"TPTREC01";

/// Why a log could not be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LogError {
    /// The log does not start with [`MAGIC`].
    BadMagic,
    /// The log ends in the middle of a record. `record` is the index of that record.
    Truncated {
        /// Zero-based index of the damaged record.
        record: usize,
    },
    /// A record's event bytes do not decode.
    Codec {
        /// Zero-based index of the damaged record.
        record: usize,
        /// What was wrong with its bytes.
        error: CodecError,
    },
}

impl fmt::Display for LogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LogError::BadMagic => f.write_str("not a tpt-window recording"),
            LogError::Truncated { record } => write!(f, "recording ends inside record {record}"),
            LogError::Codec { record, error } => write!(f, "record {record}: {error}"),
        }
    }
}

impl std::error::Error for LogError {}

/// Builds a log in memory.
#[derive(Clone, Debug)]
pub struct Recorder {
    bytes: Vec<u8>,
    records: usize,
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    /// An empty log, with the header already written.
    pub fn new() -> Self {
        Self {
            bytes: MAGIC.to_vec(),
            records: 0,
        }
    }

    /// Appends one event.
    pub fn record(&mut self, event: &Event) {
        let mut payload = Vec::with_capacity(48);
        encode_record(event.window, &event.event, &mut payload);
        self.bytes
            .extend_from_slice(&(payload.len() as u32).to_le_bytes());
        self.bytes.extend_from_slice(&payload);
        self.records += 1;
    }

    /// Drains `queue` into the log, in order. Call this after each poll, so the frame boundaries
    /// (the yield points) are kept.
    pub fn record_queue(&mut self, queue: &mut EventQueue) {
        for event in queue.drain() {
            self.record(&event);
        }
    }

    /// How many events the log holds.
    pub fn records(&self) -> usize {
        self.records
    }

    /// The log so far.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Gives the log back as bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

/// Reads every event of a log, in the order it was recorded.
pub fn read_log(bytes: &[u8]) -> Result<Vec<Event>, LogError> {
    if bytes.get(..MAGIC.len()) != Some(MAGIC.as_slice()) {
        return Err(LogError::BadMagic);
    }
    let mut pos = MAGIC.len();
    let mut out = Vec::new();
    let mut index = 0;
    while pos < bytes.len() {
        let len_end = pos + 4;
        let len_bytes = bytes
            .get(pos..len_end)
            .ok_or(LogError::Truncated { record: index })?;
        let len =
            u32::from_le_bytes([len_bytes[0], len_bytes[1], len_bytes[2], len_bytes[3]]) as usize;
        let payload = bytes
            .get(len_end..len_end + len)
            .ok_or(LogError::Truncated { record: index })?;
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
        out.push(event);
        pos = len_end + len;
        index += 1;
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PhysicalSize, WindowEvent, WindowId};

    fn sample() -> Vec<Event> {
        vec![
            Event {
                window: WindowId(1),
                event: WindowEvent::Focused(true),
            },
            Event {
                window: WindowId(2),
                event: WindowEvent::Resized(PhysicalSize::new(4, 5)),
            },
            Event {
                window: WindowId(2),
                event: WindowEvent::YieldPoint {
                    frame_number: 0,
                    timestamp_ns: 9,
                },
            },
        ]
    }

    #[test]
    fn a_log_reads_back_in_order() {
        let mut recorder = Recorder::new();
        for ev in sample() {
            recorder.record(&ev);
        }
        assert_eq!(recorder.records(), 3);
        assert_eq!(read_log(recorder.as_bytes()).unwrap(), sample());
    }

    #[test]
    fn queue_contents_are_drained_into_the_log() {
        let mut queue = EventQueue::with_capacity(8);
        for ev in sample() {
            queue.push(ev);
        }
        let mut recorder = Recorder::new();
        recorder.record_queue(&mut queue);
        assert!(queue.is_empty());
        assert_eq!(read_log(recorder.as_bytes()).unwrap(), sample());
    }

    #[test]
    fn damaged_logs_are_reported() {
        assert_eq!(read_log(b"nope"), Err(LogError::BadMagic));
        let mut recorder = Recorder::new();
        recorder.record(&sample()[0]);
        let bytes = recorder.into_bytes();
        let cut = &bytes[..bytes.len() - 1];
        assert_eq!(read_log(cut), Err(LogError::Truncated { record: 0 }));
    }
}
