// SPDX-License-Identifier: MIT OR Apache-2.0
//! The engine-facing resources (feature `ecs`): a window wrapped as a resource, and an
//! event queue that pumps the loop and yields events in arrival order.

use tpt_window_core::*;
use tpt_window_test_utils::headless;

#[test]
fn window_resource_wraps_the_window_handle() {
    let (el, _h) = headless();
    let window = WindowBuilder::new().build(&el).expect("window");
    let id = window.id();
    let res = WindowResource {
        window: window.clone(),
    };
    assert_eq!(res.window.id(), id);
    let copy = res.clone();
    assert_eq!(copy.window.id(), id);
}

#[test]
fn input_queue_pumps_and_yields_events_in_order() {
    let (mut el, h) = headless();
    let window = WindowBuilder::new().build(&el).expect("window");
    let id = window.id();
    let mut q = InputEventQueue::with_capacity(64);

    h.inject(id, WindowEvent::Focused(true));
    h.inject(id, WindowEvent::RawMouseMotion { dx: 1, dy: -2 });
    let status = q.pump(&mut el);
    assert_eq!(q.status(), status);

    let got: Vec<WindowEvent> = q
        .drain()
        .map(|e| e.event)
        .filter(|e| !matches!(e, WindowEvent::YieldPoint { .. }))
        .collect();
    assert_eq!(
        got,
        vec![
            WindowEvent::Focused(true),
            WindowEvent::RawMouseMotion { dx: 1, dy: -2 }
        ]
    );
    assert_eq!(q.dropped(), 0);
}

#[test]
fn input_queue_counts_events_lost_to_a_full_queue() {
    let (mut el, h) = headless();
    let window = WindowBuilder::new().build(&el).expect("window");
    let id = window.id();
    // Capacity 2 cannot hold the events plus the yield point, so some are lost and counted.
    let mut q = InputEventQueue::with_capacity(2);
    for i in 0..10 {
        h.inject(id, WindowEvent::RawMouseMotion { dx: i, dy: 0 });
    }
    q.pump(&mut el);
    assert!(q.dropped() > 0, "a full queue must count what it loses");
    assert!(q.drain().count() <= 2);
}
