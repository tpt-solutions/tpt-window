// SPDX-License-Identifier: MIT OR Apache-2.0
//! The yield point is how an engine or AI agent learns that a frame boundary
//! passed. These tests pin down the contract: one yield per poll, last in its
//! batch, numbered in order, and any lost yield is visible to the caller.

use std::time::Duration;

use tpt_window_core::*;
use tpt_window_test_utils::headless;

fn yields(queue: &mut EventQueue) -> Vec<u64> {
    queue
        .drain()
        .filter_map(|e| match e.event {
            WindowEvent::YieldPoint { frame_number, .. } => Some(frame_number),
            _ => None,
        })
        .collect()
}

#[test]
fn one_yield_per_poll_with_increasing_frame_numbers() {
    let (mut el, _h) = headless();
    let mut q = EventQueue::with_capacity(64);
    let mut seen = Vec::new();
    for _ in 0..5 {
        el.poll_events(&mut EventSink::new(&mut q));
        seen.extend(yields(&mut q));
    }
    assert_eq!(seen.len(), 5);
    assert!(seen.windows(2).all(|w| w[1] == w[0] + 1), "{seen:?}");
}

#[test]
fn yield_comes_after_the_os_events_of_its_frame() {
    let (mut el, h) = headless();
    let mut q = EventQueue::with_capacity(64);
    let w = WindowId(7);
    h.inject(w, WindowEvent::Focused(true));
    h.inject(w, WindowEvent::CloseRequested);
    el.poll_events(&mut EventSink::new(&mut q));
    let kinds: Vec<bool> = q
        .drain()
        .map(|e| matches!(e.event, WindowEvent::YieldPoint { .. }))
        .collect();
    assert_eq!(kinds, [false, false, true], "yield must be last");
}

#[test]
fn step_also_yields_once_even_when_idle() {
    let (mut el, _h) = headless();
    let mut q = EventQueue::with_capacity(64);
    el.step(&mut EventSink::new(&mut q), Duration::from_millis(1));
    assert_eq!(yields(&mut q).len(), 1);
}

#[test]
fn yielding_can_be_disabled() {
    let (mut el, _h) = headless();
    el.set_emit_yield_points(false);
    let mut q = EventQueue::with_capacity(64);
    el.poll_events(&mut EventSink::new(&mut q));
    assert!(yields(&mut q).is_empty());
}

#[test]
fn lost_yield_is_counted_and_leaves_a_gap_in_frame_numbers() {
    // Tiny queue, deliberately not drained between frames.
    let (mut el, h) = headless();
    let mut q = EventQueue::with_capacity(2);
    let w = WindowId(1);

    // Frame 0: one input event + yield fills the queue.
    h.inject(w, WindowEvent::Focused(true));
    el.poll_events(&mut EventSink::new(&mut q));
    assert_eq!(q.len(), 2);
    assert_eq!(q.dropped(), 0);

    // Frame 1: no room, so its yield is lost and counted.
    el.poll_events(&mut EventSink::new(&mut q));
    assert_eq!(q.dropped(), 1, "a lost yield must be counted");

    // The caller drains and sees frame 0; frame 1 is missing; frame 2 follows.
    assert_eq!(yields(&mut q), vec![0]);
    el.poll_events(&mut EventSink::new(&mut q));
    assert_eq!(yields(&mut q), vec![2], "frame 1 was lost; frame 2 is next");
}
