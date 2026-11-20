// SPDX-License-Identifier: MIT OR Apache-2.0
//! Generated window events survive the codec, and the scenario helpers deliver what they promise.
#![cfg(feature = "proptest")]

use proptest::prelude::*;
use tpt_window_core::codec::{decode_event, encode_event};
use tpt_window_core::{EventQueue, PhysicalSize, WindowBuilder, WindowEvent};
use tpt_window_test_utils::strategies::any_window_event;
use tpt_window_test_utils::{assert_events, headless, scenarios};

proptest! {
    #[test]
    fn generated_events_round_trip_through_the_codec(event in any_window_event()) {
        let mut bytes = Vec::new();
        encode_event(&event, &mut bytes);
        let (decoded, used) = decode_event(&bytes).expect("decodes");
        prop_assert_eq!(decoded, event);
        prop_assert_eq!(used, bytes.len());
    }
}

#[test]
fn a_dpi_change_is_delivered_on_its_frame() {
    let (mut el, h) = headless();
    el.set_emit_yield_points(false);
    let w = WindowBuilder::new().build(&el).unwrap();
    scenarios::dpi_change(&h, 1, w.id(), 2.0, PhysicalSize::new(1600, 1200));
    let mut q = EventQueue::with_capacity(8);
    el.poll_events(&mut tpt_window_core::EventSink::new(&mut q));
    assert!(q.is_empty(), "frame 0 is quiet");
    el.poll_events(&mut tpt_window_core::EventSink::new(&mut q));
    let events: Vec<WindowEvent> = q.drain().map(|e| e.event).collect();
    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        WindowEvent::ScaleFactorChanged { scale_factor, .. } if scale_factor == 2.0
    ));
}

#[test]
fn moving_to_a_second_display_reports_its_scale_and_position() {
    let (mut el, h) = headless();
    el.set_emit_yield_points(false);
    h.set_monitors(scenarios::two_monitors());
    let w = WindowBuilder::new().build(&el).unwrap();
    let displays = el.available_monitors();
    assert_eq!(displays.len(), 2);
    let target = displays[1].clone();
    scenarios::move_to_display(&h, 0, w.id(), &target);
    let mut q = EventQueue::with_capacity(8);
    el.poll_events(&mut tpt_window_core::EventSink::new(&mut q));
    assert_events!(
        q,
        [
            WindowEvent::ScaleFactorChanged { .. },
            WindowEvent::Moved(_)
        ]
    );
}
