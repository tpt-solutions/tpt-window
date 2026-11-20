// SPDX-License-Identifier: MIT OR Apache-2.0
//! Values sent through a proxy from another thread reach the loop in order, and a full queue
//! keeps the rest for the next poll.

use std::time::Duration;

use tpt_window_core::*;
use tpt_window_test_utils::headless;

fn users(queue: &mut EventQueue) -> Vec<u64> {
    queue
        .drain()
        .filter_map(|e| match e.event {
            WindowEvent::User(v) => Some(v),
            _ => None,
        })
        .collect()
}

#[test]
fn values_from_another_thread_arrive_in_order() {
    let (mut el, _h) = headless();
    let mut q = EventQueue::with_capacity(64);
    let proxy = el.proxy();
    std::thread::spawn(move || {
        for v in [1_u64, 2, 3] {
            proxy.send_user(v);
        }
    })
    .join()
    .unwrap();
    el.step(&mut EventSink::new(&mut q), Duration::ZERO);
    assert_eq!(users(&mut q), vec![1, 2, 3]);
}

#[test]
fn a_full_queue_keeps_the_rest_for_the_next_poll() {
    let (mut el, _h) = headless();
    // Room for the yield point plus two values, so the third waits.
    let mut q = EventQueue::with_capacity(3);
    let proxy = el.proxy();
    for v in 10..15_u64 {
        proxy.send_user(v);
    }
    let mut seen = Vec::new();
    for _ in 0..5 {
        el.poll_events(&mut EventSink::new(&mut q));
        seen.extend(users(&mut q));
    }
    assert_eq!(seen, vec![10, 11, 12, 13, 14], "nothing lost, order kept");
}
