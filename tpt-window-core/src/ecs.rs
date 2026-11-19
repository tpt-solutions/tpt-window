// SPDX-License-Identifier: MIT OR Apache-2.0
//! Engine-facing resource types (feature `ecs`).
//!
//! These are plain structs with no dependency on any particular ECS. An engine
//! such as tpt-nova inserts them as resources and drains the queue once per frame.

use crate::{DEFAULT_CAPACITY, Event, EventLoop, EventQueue, EventSink, PollStatus, Window};

/// Wraps a [`Window`] as an engine resource.
#[derive(Clone)]
pub struct WindowResource {
    /// The window handle.
    pub window: Window,
}

/// Owns the event queue and the pump that fills it, so an engine system only has
/// to call [`InputEventQueue::pump`] and then read [`InputEventQueue::drain`].
pub struct InputEventQueue {
    queue: EventQueue,
    last_status: PollStatus,
}

impl InputEventQueue {
    /// Queue with [`DEFAULT_CAPACITY`] slots.
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    /// Queue with `capacity` slots (allocated once, here).
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            queue: EventQueue::with_capacity(capacity),
            last_status: PollStatus::Idle,
        }
    }

    /// Polls the event loop into the queue without blocking.
    pub fn pump(&mut self, event_loop: &mut EventLoop) -> PollStatus {
        self.last_status = event_loop.poll_events(&mut EventSink::new(&mut self.queue));
        self.last_status
    }

    /// Status returned by the most recent [`pump`](Self::pump).
    pub fn status(&self) -> PollStatus {
        self.last_status
    }

    /// Removes and yields queued events in arrival order.
    pub fn drain(&mut self) -> impl Iterator<Item = Event> + '_ {
        self.queue.drain()
    }

    /// Events lost to a full queue since the last reset.
    pub fn dropped(&self) -> u64 {
        self.queue.dropped()
    }
}

impl Default for InputEventQueue {
    fn default() -> Self {
        Self::new()
    }
}
