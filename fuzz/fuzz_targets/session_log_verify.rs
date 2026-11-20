#![no_main]
//! Arbitrary bytes must verify to an error, never a panic. Only logs signed with the fuzzer's key
//! can verify, so this mostly checks the parser's bounds.

use ed25519_dalek::SigningKey;
use libfuzzer_sys::fuzz_target;
use tpt_window_archon::session_log::verify_session_log;

fuzz_target!(|data: &[u8]| {
    let key = SigningKey::from_bytes(&[5; 32]).verifying_key();
    let _ = verify_session_log(&key, data);
});
