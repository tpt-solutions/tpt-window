// SPDX-License-Identifier: MIT OR Apache-2.0
//! Fixed-capacity ring buffer, allocated once.

/// What to do when the buffer is full.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OverflowPolicy {
    /// Reject the new item (default); the producer sees `BufferFull`.
    #[default]
    DropNewest,
    /// Overwrite the oldest item.
    DropOldest,
}

/// Default event capacity.
pub const DEFAULT_CAPACITY: usize = 1024;

/// Caller-owned FIFO with fixed capacity. No allocation after [`RingBuffer::with_capacity`].
///
/// Single owner (`&mut self`), so no atomics are needed. This replaces the spec's
/// cross-thread SPSC description (see todo.md, Decisions).
#[derive(Debug)]
pub struct RingBuffer<T: Copy> {
    slots: Box<[Option<T>]>,
    head: usize,
    len: usize,
    dropped: u64,
    policy: OverflowPolicy,
}

impl<T: Copy> RingBuffer<T> {
    /// Allocates a buffer of `capacity` (at least 1) slots.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity.max(1)].into_boxed_slice(),
            head: 0,
            len: 0,
            dropped: 0,
            policy: OverflowPolicy::default(),
        }
    }

    /// Sets the overflow policy.
    pub fn with_policy(mut self, policy: OverflowPolicy) -> Self {
        self.policy = policy;
        self
    }

    /// Total slots.
    pub fn capacity(&self) -> usize {
        self.slots.len()
    }

    /// Items currently stored.
    pub fn len(&self) -> usize {
        self.len
    }

    /// True if empty.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// True if full.
    pub fn is_full(&self) -> bool {
        self.len == self.slots.len()
    }

    /// Items actually lost since creation or the last [`reset_dropped`](Self::reset_dropped):
    /// `DropOldest` evictions plus anything a producer reports via
    /// [`record_drop`](Self::record_drop). A rejected `DropNewest` push is *not* counted,
    /// because the producer may keep the item and retry.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// Records that a producer gave up on an item the buffer rejected.
    pub fn record_drop(&mut self) {
        self.dropped += 1;
    }

    /// Resets the drop counter.
    pub fn reset_dropped(&mut self) {
        self.dropped = 0;
    }

    /// Returns the drop count and resets it to zero, so each loss is reported once.
    pub fn take_dropped(&mut self) -> u64 {
        std::mem::take(&mut self.dropped)
    }

    /// Pushes an item. Returns `false` if it was not stored (`DropNewest` and full).
    /// With `DropOldest` this always returns `true` and may evict the oldest item.
    pub fn push(&mut self, item: T) -> bool {
        if self.is_full() {
            match self.policy {
                OverflowPolicy::DropNewest => return false,
                OverflowPolicy::DropOldest => {
                    self.dropped += 1;
                    self.head = (self.head + 1) % self.slots.len();
                    self.len -= 1;
                }
            }
        }
        let idx = (self.head + self.len) % self.slots.len();
        self.slots[idx] = Some(item);
        self.len += 1;
        true
    }

    /// The oldest item, without removing it.
    pub fn front(&self) -> Option<T> {
        if self.len == 0 {
            None
        } else {
            self.slots[self.head]
        }
    }

    /// Removes and returns the oldest item.
    pub fn pop(&mut self) -> Option<T> {
        if self.len == 0 {
            return None;
        }
        let item = self.slots[self.head].take();
        self.head = (self.head + 1) % self.slots.len();
        self.len -= 1;
        item
    }

    /// Drains all items oldest-first.
    pub fn drain(&mut self) -> Drain<'_, T> {
        Drain { ring: self }
    }

    /// Removes everything (does not touch the drop counter).
    pub fn clear(&mut self) {
        while self.pop().is_some() {}
    }
}

/// Iterator returned by [`RingBuffer::drain`].
pub struct Drain<'a, T: Copy> {
    ring: &'a mut RingBuffer<T>,
}

impl<T: Copy> Iterator for Drain<'_, T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.ring.pop()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_and_wrap() {
        let mut r = RingBuffer::with_capacity(3);
        for i in 0..3 {
            assert!(r.push(i));
        }
        assert_eq!(r.pop(), Some(0));
        assert!(r.push(3)); // wraps
        assert_eq!(r.drain().collect::<Vec<_>>(), vec![1, 2, 3]);
        assert!(r.is_empty());
    }

    #[test]
    fn drop_newest() {
        let mut r = RingBuffer::with_capacity(2);
        assert!(r.push(1) && r.push(2));
        assert!(!r.push(3));
        assert_eq!(r.dropped(), 0); // rejected, not lost: the producer may retry
        r.record_drop();
        assert_eq!(r.dropped(), 1);
        assert_eq!(r.drain().collect::<Vec<_>>(), vec![1, 2]);
    }

    #[test]
    fn drop_oldest() {
        let mut r = RingBuffer::with_capacity(2).with_policy(OverflowPolicy::DropOldest);
        assert!(r.push(1) && r.push(2) && r.push(3));
        assert_eq!(r.dropped(), 1);
        assert_eq!(r.drain().collect::<Vec<_>>(), vec![2, 3]);
    }

    #[test]
    fn capacity_floor() {
        assert_eq!(RingBuffer::<u8>::with_capacity(0).capacity(), 1);
    }
}
