#![no_main]
//! Random bytes in a ring's memory, and random push and pop sequences, must never panic or move
//! the consumer past a corrupt frame.

use libfuzzer_sys::fuzz_target;
use tpt_window_core::channel::{FrameRing, RingMemory};
use tpt_window_core::protocol::Message;

struct Mem(Vec<u8>);

impl RingMemory for Mem {
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
        u32::from_le_bytes(self.0[offset..offset + 4].try_into().unwrap())
    }
    fn store_u32(&mut self, offset: usize, value: u32) {
        self.0[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
}

fuzz_target!(|data: &[u8]| {
    // The first byte picks the memory size class; the rest drive operations.
    let size = 8192 + 16 + usize::from(data.first().copied().unwrap_or(0)) * 64;
    let Ok(mut ring) = FrameRing::new(Mem(vec![0; size])) else {
        return;
    };
    ring.reset();
    for &op in data.iter().skip(1) {
        if op & 1 == 0 {
            let _ = ring.push(&Message::Hello { version: u32::from(op) });
        } else {
            let _ = ring.pop();
        }
    }
});
