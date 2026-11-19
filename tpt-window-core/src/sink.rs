// SPDX-License-Identifier: MIT OR Apache-2.0
//! The write side of the event queue handed to backends each poll.

use crate::event::Event;
use crate::ring::RingBuffer;

/// Queue type used for window events.
pub type EventQueue = RingBuffer<Event>;

/// Result of [`EventSink::push`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventStatus {
    /// Event was accepted.
    Accepted,
    /// Buffer full (and policy is drop-newest). The backend should keep the
    /// event and retry on its next poll after the caller drains the queue.
    BufferFull,
    /// The application captured this poll; the event was not stored and the
    /// backend should stop delivering for this poll.
    Captured,
}

/// Short-lived writer over a caller-owned [`EventQueue`]. `push` never allocates.
pub struct EventSink<'a> {
    buffer: &'a mut EventQueue,
    captured: usize,
    capture_requested: bool,
}

impl<'a> EventSink<'a> {
    /// Wraps a queue for one poll.
    pub fn new(buffer: &'a mut EventQueue) -> Self {
        Self {
            buffer,
            captured: 0,
            capture_requested: false,
        }
    }

    /// Pushes an event.
    pub fn push(&mut self, event: Event) -> EventStatus {
        if self.capture_requested {
            return EventStatus::Captured;
        }
        if self.buffer.push(event) {
            self.captured += 1;
            EventStatus::Accepted
        } else {
            EventStatus::BufferFull
        }
    }

    /// Number of events accepted during this poll.
    pub fn captured_count(&self) -> usize {
        self.captured
    }

    /// Asks backends to stop delivering for the rest of this poll.
    pub fn capture(&mut self) {
        self.capture_requested = true;
    }

    /// Whether [`capture`](Self::capture) was called.
    pub fn is_captured(&self) -> bool {
        self.capture_requested
    }

    /// Reports that the backend gave up on an event after `BufferFull`
    /// (counted in [`RingBuffer::dropped`]).
    pub fn record_drop(&mut self) {
        self.buffer.record_drop();
    }

    /// Free slots remaining in the queue.
    pub fn remaining(&self) -> usize {
        self.buffer.capacity() - self.buffer.len()
    }
}
