//! Tests for window-driven application logic, run without a display.
//!
//! Put the logic under test in functions that take events, and script the events here.

use tpt_window_core::WindowEvent;

/// The logic under test: a counter that counts focus gains.
#[derive(Default)]
pub struct Counter {
    pub focus_gains: u32,
}

impl Counter {
    pub fn on_event(&mut self, event: &WindowEvent) {
        if *event == WindowEvent::Focused(true) {
            self.focus_gains += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_window_core::{EventQueue, EventSink, PollStatus, WindowBuilder};
    use tpt_window_test_utils::{assert_events, headless};

    #[test]
    fn focus_gains_are_counted_on_their_frames() {
        let (mut event_loop, handle) = headless();
        event_loop.set_emit_yield_points(false);
        let window = WindowBuilder::new().build(&event_loop).expect("window");
        handle.inject_at(0, window.id(), WindowEvent::Focused(true));
        handle.inject_at(1, window.id(), WindowEvent::Focused(false));
        handle.inject_at(2, window.id(), WindowEvent::Focused(true));

        let mut counter = Counter::default();
        let mut queue = EventQueue::with_capacity(16);
        for _ in 0..3 {
            event_loop.poll_events(&mut EventSink::new(&mut queue));
            for ev in queue.drain() {
                counter.on_event(&ev.event);
            }
        }
        assert_eq!(counter.focus_gains, 2);

        // The loop reports exit once asked, so a test can end its own run.
        event_loop.request_exit();
        let status = event_loop.poll_events(&mut EventSink::new(&mut queue));
        assert_eq!(status, PollStatus::Exit);
        assert_events!(queue, []);
    }
}
