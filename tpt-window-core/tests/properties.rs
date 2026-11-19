// SPDX-License-Identifier: MIT OR Apache-2.0
//! Property tests for the ring buffer and inline text.

use std::collections::VecDeque;

use proptest::prelude::*;
use tpt_window_core::*;

#[derive(Debug, Clone)]
enum Op {
    Push(u32),
    Pop,
}

fn ops() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(
        prop_oneof![any::<u32>().prop_map(Op::Push), Just(Op::Pop)],
        0..200,
    )
}

proptest! {
    /// DropNewest behaves like a bounded FIFO that rejects pushes when full.
    #[test]
    fn drop_newest_matches_model(cap in 1usize..16, ops in ops()) {
        let mut ring = RingBuffer::<u32>::with_capacity(cap);
        let mut model = VecDeque::new();
        for op in ops {
            match op {
                Op::Push(v) => {
                    let ok = ring.push(v);
                    prop_assert_eq!(ok, model.len() < cap);
                    if ok { model.push_back(v); }
                }
                Op::Pop => prop_assert_eq!(ring.pop(), model.pop_front()),
            }
            prop_assert_eq!(ring.len(), model.len());
            prop_assert!(ring.len() <= cap);
        }
        prop_assert_eq!(ring.dropped(), 0);
    }

    /// DropOldest keeps the newest `cap` items and counts every eviction.
    #[test]
    fn drop_oldest_matches_model(cap in 1usize..16, ops in ops()) {
        let mut ring = RingBuffer::<u32>::with_capacity(cap).with_policy(OverflowPolicy::DropOldest);
        let mut model = VecDeque::new();
        let mut evicted = 0u64;
        for op in ops {
            match op {
                Op::Push(v) => {
                    if model.len() == cap { model.pop_front(); evicted += 1; }
                    model.push_back(v);
                    prop_assert!(ring.push(v));
                }
                Op::Pop => prop_assert_eq!(ring.pop(), model.pop_front()),
            }
        }
        prop_assert_eq!(ring.dropped(), evicted);
        prop_assert_eq!(ring.drain().collect::<Vec<_>>(), model.into_iter().collect::<Vec<_>>());
    }

    /// Chunking never splits a character, never loses bytes, and respects capacity.
    #[test]
    fn inline_text_chunks_reassemble(s in r"\PC{0,200}") {
        let mut rest = s.as_str();
        let mut out = String::new();
        while !rest.is_empty() {
            let (chunk, next) = InlineText::<32>::take_chunk(rest);
            prop_assert!(chunk.len() <= 32);
            // A 4-byte character always fits in 32 bytes, so progress is guaranteed.
            prop_assert!(!chunk.is_empty());
            out.push_str(chunk.as_str());
            rest = next;
        }
        prop_assert_eq!(out, s);
    }

    /// `new` truncates on a character boundary and yields a prefix.
    #[test]
    fn inline_text_new_is_prefix(s in r"\PC{0,100}") {
        let t = InlineText::<8>::new(&s);
        prop_assert!(t.len() <= 8);
        prop_assert!(s.starts_with(t.as_str()));
    }
}
