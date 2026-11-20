// SPDX-License-Identifier: MIT OR Apache-2.0
//! Single-producer, single-consumer ring of protocol frames over shared memory.
//!
//! The ring logic is safe Rust. It works on a [`RingMemory`], which is shared memory on a
//! real platform (an OS mapping, provided by a platform crate) or a `Vec` in tests. Both
//! sides need the same memory and must agree on its size.
//!
//! Layout inside the memory:
//!
//! ```text
//! [0..4)   head: bytes written so far (producer), u32 LE, wraps modulo 2^32
//! [4..8)   tail: bytes consumed so far (consumer), u32 LE, wraps modulo 2^32
//! [8..16)  reserved, zero
//! [16..)   data: frames in the wire format of `protocol`, wrapping at the end
//! ```
//!
//! Ordering is the platform's job. `store_u32` must be a release store and `load_u32` an
//! acquire load, and the data bytes must be written before the head store. Without that,
//! the consumer can read a frame before it is complete.
//!
//! A ring over plain memory, as a test or an in-process pipe would use. A real transport
//! supplies a `RingMemory` backed by a shared mapping (see `tpt-window-shm` and
//! `tpt-window-memfd`):
//!
//! ```
//! use tpt_window_core::channel::{FrameRing, RingMemory};
//! use tpt_window_core::protocol::Message;
//!
//! struct Memory(Vec<u8>);
//!
//! impl RingMemory for Memory {
//!     fn len(&self) -> usize { self.0.len() }
//!     fn read(&self, offset: usize, out: &mut [u8]) {
//!         out.copy_from_slice(&self.0[offset..offset + out.len()]);
//!     }
//!     fn write(&mut self, offset: usize, data: &[u8]) {
//!         self.0[offset..offset + data.len()].copy_from_slice(data);
//!     }
//!     fn load_u32(&self, offset: usize) -> u32 {
//!         u32::from_le_bytes(self.0[offset..offset + 4].try_into().unwrap())
//!     }
//!     fn store_u32(&mut self, offset: usize, value: u32) {
//!         self.0[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
//!     }
//! }
//!
//! let mut ring = FrameRing::new(Memory(vec![0; 8192 + 16])).unwrap();
//! ring.reset();
//! ring.push(&Message::Hello { version: 1 }).unwrap();
//! assert_eq!(ring.pop().unwrap(), Some(Message::Hello { version: 1 }));
//! ```

use crate::protocol::{self, DecodeError, Message};

/// Bytes at the start of the memory reserved for the indices.
pub const HEADER_LEN: usize = 16;

const HEAD: usize = 0;
const TAIL: usize = 4;

/// Memory a ring lives in.
pub trait RingMemory {
    /// Size of the memory in bytes.
    fn len(&self) -> usize;
    /// Whether the memory has no bytes.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// Copies `out.len()` bytes starting at `offset`.
    fn read(&self, offset: usize, out: &mut [u8]);
    /// Writes `data` starting at `offset`.
    fn write(&mut self, offset: usize, data: &[u8]);
    /// Loads a 32-bit value at a 4-byte-aligned `offset` with acquire ordering.
    fn load_u32(&self, offset: usize) -> u32;
    /// Stores a 32-bit value at a 4-byte-aligned `offset` with release ordering.
    fn store_u32(&mut self, offset: usize, value: u32);
}

/// Why a ring operation failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RingError {
    /// The memory is too small to hold the header and at least one maximum-size frame.
    TooSmall {
        /// The memory size given.
        len: usize,
    },
    /// Not enough free space for the frame. Retry after the consumer has popped.
    Full,
    /// The bytes in the ring are not a valid frame (a corrupted or hostile producer).
    Corrupt(DecodeError),
}

impl core::fmt::Display for RingError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RingError::TooSmall { len } => write!(f, "ring memory of {len} bytes is too small"),
            RingError::Full => f.write_str("ring is full"),
            RingError::Corrupt(e) => write!(f, "corrupt frame in ring: {e}"),
        }
    }
}

impl std::error::Error for RingError {}

/// Largest frame on the wire, length prefix included.
const MAX_FRAME_BYTES: usize = 4 + protocol::MAX_FRAME_LEN as usize;

/// Data capacity for a memory of `len` bytes: the largest power of two that fits.
///
/// The indices are `u32` and wrap modulo 2^32. A power of two divides 2^32, so the offset
/// `index % capacity` stays continuous across the wrap. A capacity such as 4100 does not,
/// and frames would land at the wrong offset after 4 GiB of traffic.
fn usable_capacity(len: usize) -> usize {
    let data = len.saturating_sub(HEADER_LEN);
    if data == 0 {
        return 0;
    }
    // Cap at 2^31 so that `queued_bytes` stays unambiguous under wrapping subtraction.
    let limit = 1usize << 31;
    1usize << (usize::BITS - 1 - data.min(limit).leading_zeros())
}

/// A ring of protocol frames over a [`RingMemory`].
pub struct FrameRing<M: RingMemory> {
    mem: M,
    capacity: usize,
    /// Reused by `push` and `pop`, so a steady stream of frames does not allocate.
    frame_buf: Vec<u8>,
}

impl<M: RingMemory> FrameRing<M> {
    /// Wraps `mem`. The data area is the largest power of two that fits after the header,
    /// and it must hold one maximum-size frame, so the memory needs at least
    /// `HEADER_LEN + 8192` bytes. Call [`reset`](Self::reset) once, on the side that creates
    /// the memory, before use.
    pub fn new(mem: M) -> Result<Self, RingError> {
        let capacity = usable_capacity(mem.len());
        if capacity < MAX_FRAME_BYTES {
            return Err(RingError::TooSmall { len: mem.len() });
        }
        Ok(Self {
            mem,
            capacity,
            frame_buf: Vec::with_capacity(MAX_FRAME_BYTES),
        })
    }

    /// Empties the ring. Only the creating side should call this.
    pub fn reset(&mut self) {
        self.mem.store_u32(HEAD, 0);
        self.mem.store_u32(TAIL, 0);
    }

    /// Bytes of frame data the ring can hold.
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Bytes currently queued.
    pub fn queued_bytes(&self) -> usize {
        let head = self.mem.load_u32(HEAD);
        let tail = self.mem.load_u32(TAIL);
        head.wrapping_sub(tail) as usize
    }

    /// Appends `message`. Fails with [`RingError::Full`] when the frame does not fit, and
    /// then changes nothing.
    pub fn push(&mut self, message: &Message) -> Result<(), RingError> {
        let mut frame = core::mem::take(&mut self.frame_buf);
        frame.clear();
        protocol::encode(message, &mut frame);
        let result = self.push_encoded(&frame);
        self.frame_buf = frame;
        result
    }

    fn push_encoded(&mut self, frame: &[u8]) -> Result<(), RingError> {
        let cap = self.capacity();
        if frame.len() > cap.saturating_sub(self.queued_bytes()) {
            return Err(RingError::Full);
        }
        let head = self.mem.load_u32(HEAD);
        self.write_wrapping(head as usize, frame);
        self.mem
            .store_u32(HEAD, head.wrapping_add(frame.len() as u32));
        Ok(())
    }

    /// Removes the oldest frame, or returns `Ok(None)` when the ring is empty.
    ///
    /// On [`RingError::Corrupt`] the consumer position does not move, so the bad bytes are
    /// not skipped silently. The caller should reset the ring or close the session.
    pub fn pop(&mut self) -> Result<Option<Message>, RingError> {
        let mut frame = core::mem::take(&mut self.frame_buf);
        let result = self.pop_into(&mut frame);
        self.frame_buf = frame;
        result
    }

    fn pop_into(&mut self, frame: &mut Vec<u8>) -> Result<Option<Message>, RingError> {
        let queued = self.queued_bytes();
        if queued == 0 {
            return Ok(None);
        }
        let tail = self.mem.load_u32(TAIL);
        let mut len_bytes = [0u8; 4];
        self.read_wrapping(tail as usize, &mut len_bytes);
        let body_len = u32::from_le_bytes(len_bytes);
        if body_len > protocol::MAX_FRAME_LEN {
            return Err(RingError::Corrupt(DecodeError::TooLong(body_len)));
        }
        let frame_len = 4 + body_len as usize;
        if frame_len > queued {
            return Err(RingError::Corrupt(DecodeError::Truncated));
        }
        frame.clear();
        frame.resize(frame_len, 0);
        self.read_wrapping(tail as usize, frame);
        let (message, used) = protocol::decode(frame).map_err(RingError::Corrupt)?;
        debug_assert_eq!(used, frame_len);
        self.mem
            .store_u32(TAIL, tail.wrapping_add(frame_len as u32));
        Ok(Some(message))
    }

    /// Gives back the memory.
    pub fn into_memory(self) -> M {
        self.mem
    }

    fn data_offset(&self, position: usize) -> usize {
        HEADER_LEN + position % self.capacity()
    }

    fn write_wrapping(&mut self, position: usize, data: &[u8]) {
        let cap = self.capacity();
        let start = position % cap;
        let first = data.len().min(cap - start);
        self.mem.write(HEADER_LEN + start, &data[..first]);
        if first < data.len() {
            self.mem.write(HEADER_LEN, &data[first..]);
        }
    }

    fn read_wrapping(&self, position: usize, out: &mut [u8]) {
        let cap = self.capacity();
        let start = position % cap;
        let first = out.len().min(cap - start);
        self.mem.read(self.data_offset(position), &mut out[..first]);
        if first < out.len() {
            self.mem.read(HEADER_LEN, &mut out[first..]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PhysicalSize, SurfaceHandle, WindowId};

    /// Plain memory for tests. Single-threaded, so ordering needs no care here.
    struct VecMemory(Vec<u8>);

    impl RingMemory for VecMemory {
        fn len(&self) -> usize {
            self.0.len()
        }
        fn read(&self, offset: usize, out: &mut [u8]) {
            out.copy_from_slice(&self.0[offset..offset + out.len()]);
        }
        fn write(&mut self, offset: usize, data: &[u8]) {
            self.0[offset..offset + data.len()].copy_from_slice(data);
        }
        fn load_u32(&self, offset: usize) -> u32 {
            let mut b = [0u8; 4];
            b.copy_from_slice(&self.0[offset..offset + 4]);
            u32::from_le_bytes(b)
        }
        fn store_u32(&mut self, offset: usize, value: u32) {
            self.0[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
    }

    /// The smallest memory a ring accepts: the header and a power-of-two data area that holds
    /// one maximum-size frame (4100 bytes rounds up to 8192).
    const MIN: usize = HEADER_LEN + 8192;

    fn ring(len: usize) -> FrameRing<VecMemory> {
        let mut r = FrameRing::new(VecMemory(vec![0xAB; len])).expect("big enough");
        r.reset();
        r
    }

    fn frame(i: u64) -> Message {
        Message::FramePresented {
            window: WindowId(i),
            surface: SurfaceHandle { id: i * 3 },
            frame: i * 7,
        }
    }

    #[test]
    fn memory_must_hold_a_maximum_frame() {
        let small = VecMemory(vec![0; HEADER_LEN + 10]);
        assert!(matches!(
            FrameRing::new(small),
            Err(RingError::TooSmall { .. })
        ));
    }

    #[test]
    fn data_area_is_a_power_of_two_that_fits_a_maximum_frame() {
        // 4100 bytes of data would be the naive minimum, but 2^32 % 4100 != 0 breaks the
        // wrap, so it is rounded down to 4096 and rejected.
        let just_short = HEADER_LEN + MAX_FRAME_BYTES;
        assert!(matches!(
            FrameRing::new(VecMemory(vec![0; just_short])),
            Err(RingError::TooSmall { .. })
        ));
        // A non-power-of-two memory keeps only the largest power of two.
        let r = ring(HEADER_LEN + 10_000);
        assert_eq!(r.capacity(), 8192);
        assert_eq!(2u64.pow(32) % r.capacity() as u64, 0);
    }

    #[test]
    fn indices_wrapping_past_u32_max_keep_frames_in_place() {
        // Start both indices a few hundred bytes below 2^32 so the traffic crosses the wrap.
        // With a capacity that does not divide 2^32, this put frames at the wrong offset.
        let start = u32::MAX - 500;
        let mut mem = VecMemory(vec![0xAB; MIN + 100]);
        mem.store_u32(HEAD, start);
        mem.store_u32(TAIL, start);
        let mut r = FrameRing::new(mem).unwrap();
        for i in 0..200u64 {
            r.push(&frame(i)).unwrap();
            assert_eq!(r.pop().unwrap(), Some(frame(i)), "frame {i}");
        }
        // Positions really did wrap past zero.
        let mem = r.into_memory();
        assert!(mem.load_u32(HEAD) < start, "head must have wrapped");
    }

    #[test]
    fn frames_come_out_in_order() {
        let mut r = ring(MIN);
        let msgs = [
            Message::Hello { version: 1 },
            Message::SurfaceReady {
                window: WindowId(2),
                surface: SurfaceHandle { id: 5 },
                size: PhysicalSize::new(640, 360),
            },
            frame(9),
        ];
        for m in &msgs {
            r.push(m).unwrap();
        }
        for m in &msgs {
            assert_eq!(r.pop().unwrap(), Some(*m));
        }
        assert_eq!(r.pop().unwrap(), None);
        assert_eq!(r.queued_bytes(), 0);
    }

    #[test]
    fn wraps_around_the_end_of_the_memory_many_times() {
        // A small ring forces every frame to split across the end, many times over. The
        // indices also wrap their 32-bit range after enough traffic.
        let mut r = ring(MIN);
        for i in 0..50_000u64 {
            r.push(&frame(i)).unwrap();
            assert_eq!(r.pop().unwrap(), Some(frame(i)), "frame {i}");
        }
    }

    #[test]
    fn full_ring_refuses_without_changing_anything() {
        let mut r = ring(MIN);
        let mut pushed = 0;
        loop {
            match r.push(&frame(pushed)) {
                Ok(()) => pushed += 1,
                Err(RingError::Full) => break,
                Err(e) => panic!("unexpected {e}"),
            }
        }
        assert!(pushed > 0);
        let before = r.queued_bytes();
        assert_eq!(r.push(&frame(999)), Err(RingError::Full));
        assert_eq!(r.queued_bytes(), before);
        // Everything that was accepted is still there, in order.
        for i in 0..pushed {
            assert_eq!(r.pop().unwrap(), Some(frame(i)));
        }
    }

    #[test]
    fn corrupt_length_is_reported_and_does_not_advance() {
        let mut r = ring(MIN);
        r.push(&frame(1)).unwrap();
        // Overwrite the length prefix of the queued frame with an oversized value.
        let mut mem = r.into_memory();
        mem.write(HEADER_LEN, &(protocol::MAX_FRAME_LEN + 1).to_le_bytes());
        mem.store_u32(HEAD, 40);
        let mut r = FrameRing::new(mem).unwrap();
        assert!(matches!(r.pop(), Err(RingError::Corrupt(_))));
        assert!(
            matches!(r.pop(), Err(RingError::Corrupt(_))),
            "position must not move"
        );
    }
}
