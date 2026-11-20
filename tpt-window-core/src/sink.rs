// SPDX-License-Identifier: MIT OR Apache-2.0
//! The write side of the event queue handed to backends each poll.

use crate::event::Event;
use crate::ring::{OverflowPolicy, RingBuffer};

/// Queue type used for window events.
pub type EventQueue = RingBuffer<Event>;

/// Slots in a backend's pending queue: events the OS produced and the caller has not yet
/// drained. Full pending queues evict the oldest event, and the eviction is counted.
pub const PENDING_CAPACITY: usize = 4096;

/// Creates the bounded pending queue every backend uses between the OS and the caller.
/// Allocates once; it never grows, so a stalled caller cannot exhaust memory.
pub fn new_pending_queue() -> EventQueue {
    RingBuffer::with_capacity(PENDING_CAPACITY).with_policy(OverflowPolicy::DropOldest)
}

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
    pub fn accepted_count(&self) -> usize {
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

    /// Moves a backend's pending events into this sink, oldest first. Stops at the first
    /// event the sink refuses (full or captured); the rest stay in `pending`.
    ///
    /// Evictions from `pending` since the last drain (see [`RingBuffer::dropped`]) are
    /// recorded in this sink's drop count, so the caller sees them in `dropped()`. Returns
    /// whether at least one event was delivered.
    pub fn drain_pending(&mut self, pending: &mut EventQueue) -> bool {
        for _ in 0..pending.take_dropped() {
            self.buffer.record_drop();
        }
        let mut delivered = false;
        while let Some(ev) = pending.front() {
            match self.push(ev) {
                EventStatus::Accepted => {
                    pending.pop();
                    delivered = true;
                }
                EventStatus::BufferFull | EventStatus::Captured => break,
            }
        }
        delivered
    }

    /// Free slots remaining in the queue.
    pub fn remaining(&self) -> usize {
        self.buffer.capacity() - self.buffer.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{WindowEvent, WindowId};

    fn ev(i: u64) -> Event {
        Event {
            window: WindowId(i),
            event: WindowEvent::Focused(true),
        }
    }

    #[test]
    fn pending_evictions_show_up_as_caller_drops() {
        // A backend that produced more events than its pending queue holds, while the caller
        // did not drain. The oldest 10 are evicted and must be counted, not vanish.
        let mut pending = new_pending_queue();
        for i in 0..(PENDING_CAPACITY as u64 + 10) {
            pending.push(ev(i));
        }
        assert_eq!(pending.dropped(), 10);

        let mut caller = RingBuffer::with_capacity(PENDING_CAPACITY * 2);
        let delivered = EventSink::new(&mut caller).drain_pending(&mut pending);
        assert!(delivered);
        assert_eq!(caller.dropped(), 10, "evictions reach the caller's queue");
        assert_eq!(caller.len(), PENDING_CAPACITY);
        assert!(pending.is_empty());
        assert_eq!(pending.dropped(), 0, "each loss is reported once");
        // The first event the caller sees is the oldest one that survived.
        assert_eq!(caller.pop().map(|e| e.window), Some(WindowId(10)));
    }

    #[test]
    fn full_caller_queue_keeps_the_rest_pending() {
        let mut pending = new_pending_queue();
        for i in 0..5 {
            pending.push(ev(i));
        }
        let mut caller = RingBuffer::with_capacity(3);
        let delivered = EventSink::new(&mut caller).drain_pending(&mut pending);
        assert!(delivered);
        assert_eq!(caller.len(), 3);
        assert_eq!(pending.len(), 2, "refused events stay queued, not lost");
        assert_eq!(caller.dropped(), 0);
    }
}
