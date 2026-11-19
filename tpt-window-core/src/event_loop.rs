// SPDX-License-Identifier: MIT OR Apache-2.0
//! The caller-driven event loop.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::backend::{EventLoopBackend, PollStatus, WindowAttributes};
use crate::error::WindowError;
use crate::event::{Event, WindowEvent};
use crate::ids::WindowId;
use crate::sink::EventSink;
use crate::window::Window;

/// Per-thread message pump. The caller owns the main loop and calls
/// [`poll_events`](Self::poll_events) or [`step`](Self::step) itself.
///
/// The platform-specific constructor lives in the `tpt-window` facade
/// (`tpt_window::new_event_loop()`), because core cannot name a backend.
pub struct EventLoop {
    inner: Box<dyn EventLoopBackend>,
    exit: AtomicBool,
    emit_yield_points: bool,
    frame: u64,
    start: Instant,
}

impl EventLoop {
    /// Wraps a backend.
    pub fn with_backend(inner: Box<dyn EventLoopBackend>) -> Self {
        Self {
            inner,
            exit: AtomicBool::new(false),
            emit_yield_points: true,
            frame: 0,
            start: Instant::now(),
        }
    }

    /// Enable or disable automatic [`WindowEvent::YieldPoint`] emission (default on).
    pub fn set_emit_yield_points(&mut self, on: bool) {
        self.emit_yield_points = on;
    }

    /// Non-blocking poll. Delivers pending OS events, then one `YieldPoint`
    /// (the frame boundary). Never allocates.
    pub fn poll_events(&mut self, sink: &mut EventSink<'_>) -> PollStatus {
        let status = self.inner.poll(sink);
        self.finish_frame(sink, status)
    }

    /// Blocking step: waits up to `timeout` for events, then behaves like
    /// [`poll_events`](Self::poll_events).
    pub fn step(&mut self, sink: &mut EventSink<'_>, timeout: Duration) -> PollStatus {
        let status = self.inner.wait(sink, timeout);
        self.finish_frame(sink, status)
    }

    /// Ask the loop to terminate; the next poll returns [`PollStatus::Exit`].
    pub fn request_exit(&self) {
        self.exit.store(true, Ordering::Release);
        self.inner.request_exit();
    }

    pub(crate) fn create_window(&self, attrs: &WindowAttributes) -> Result<Window, WindowError> {
        self.inner.create_window(attrs).map(Window::from_backend)
    }

    fn finish_frame(&mut self, sink: &mut EventSink<'_>, status: PollStatus) -> PollStatus {
        if self.emit_yield_points {
            let timestamp_ns = u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX);
            // If the queue is full the yield point is lost; the caller must drain.
            let _ = sink.push(Event {
                window: WindowId::NONE,
                event: WindowEvent::YieldPoint {
                    frame_number: self.frame,
                    timestamp_ns,
                },
            });
        }
        self.frame += 1;
        if self.exit.load(Ordering::Acquire) {
            PollStatus::Exit
        } else {
            status
        }
    }
}
