#![no_main]
//! Arbitrary bytes must decode to an event or an error, never a panic, and a successful decode
//! must re-encode to the same bytes it consumed.

use libfuzzer_sys::fuzz_target;
use tpt_window_core::codec::{decode_event, encode_event};

fuzz_target!(|data: &[u8]| {
    if let Ok((event, used)) = decode_event(data) {
        let mut again = Vec::new();
        encode_event(&event, &mut again);
        assert_eq!(again.as_slice(), &data[..used], "re-encoding must reproduce the bytes");
    }
});
