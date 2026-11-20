#![no_main]
//! Arbitrary bytes must decode to a protocol message or an error, never a panic, and the decoder
//! must not allocate for a declared length above the limit.

use libfuzzer_sys::fuzz_target;
use tpt_window_core::protocol::decode;

fuzz_target!(|data: &[u8]| {
    let _ = decode(data);
});
