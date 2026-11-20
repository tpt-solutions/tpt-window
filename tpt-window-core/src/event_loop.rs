// SPDX-License-Identifier: MIT OR Apache-2.0
//! The caller-driven event loop.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;
// The browser has no std clock; web-time reads performance.now() instead.
#[cfg(target_arch = "wasm32")]
use web_time::Instant;

use crate::backend::{EventLoopBackend, PollStatus, WindowAttributes};
use crate::error::WindowError;
use crate::event::{Event, WindowEvent};
use crate::ids::WindowId;
use crate::monitor::MonitorInfo;
use crate::proxy::EventLoopProxy;
use crate::sink::{EventSink, EventStatus};
use crate::window::Window;

/// Per-thread message pump. The caller owns the main loop and calls
/// [`poll_events`](Self::poll_events) or [`step`](Self::step) itself.
///
/// The platform-specific constructor lives in the `tpt-window` facade
/// (`tpt_window::new_event_loop()`), because core cannot name a backend.
pub struct EventLoop {
    inner: Box<dyn EventLoopBackend>,
    exit: Arc<AtomicBool>,
    /// Values sent by [`EventLoopProxy::send_user`], waiting for the next poll.
    user: Arc<Mutex<VecDeque<u64>>>,
    emit_yield_points: bool,
    frame: u64,
    start: Instant,
}

impl EventLoop {
    /// Wraps a backend.
    pub fn with_backend(inner: Box<dyn EventLoopBackend>) -> Self {
        Self {
            inner,
            exit: Arc::new(AtomicBool::new(false)),
            user: Arc::new(Mutex::new(VecDeque::new())),
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

    /// The displays attached right now. Empty on backends that cannot enumerate them.
    pub fn available_monitors(&self) -> Vec<MonitorInfo> {
        self.inner.monitors()
    }

    /// A handle for other threads to wake this loop or stop it. See [`EventLoopProxy`].
    pub fn proxy(&self) -> EventLoopProxy {
        let wake = self
            .inner
            .wake_handle()
            .unwrap_or_else(|| Arc::new(crate::proxy::NoWake));
        EventLoopProxy::new(wake, self.exit.clone(), self.user.clone())
    }

    /// Asks the loop to terminate; the next poll returns [`PollStatus::Exit`]. Call it from the
    /// thread that owns the loop, since `EventLoop` is not `Send`.
    pub fn request_exit(&self) {
        self.exit.store(true, Ordering::Release);
        self.inner.request_exit();
    }

    pub(crate) fn create_window(&self, attrs: &WindowAttributes) -> Result<Window, WindowError> {
        self.inner.create_window(attrs).map(Window::from_backend)
    }

    /// Moves user values into the sink in the order they were sent. When the sink is full, the
    /// rest stay queued for the next poll, so nothing is lost.
    fn deliver_user(&self, sink: &mut EventSink<'_>) {
        let mut queue = self.user.lock().unwrap_or_else(|e| e.into_inner());
        while let Some(&value) = queue.front() {
            let event = Event {
                window: WindowId::NONE,
                event: WindowEvent::User(value),
            };
            if sink.push(event) != EventStatus::Accepted {
                break;
            }
            queue.pop_front();
        }
    }

    fn finish_frame(&mut self, sink: &mut EventSink<'_>, status: PollStatus) -> PollStatus {
        self.deliver_user(sink);
        if self.emit_yield_points {
            let timestamp_ns = u64::try_from(self.start.elapsed().as_nanos()).unwrap_or(u64::MAX);
            // If the queue is full the yield point is lost. That is counted as a
            // drop, and the frame number keeps counting, so the caller sees a gap
            // in `frame_number` and knows it missed a frame boundary.
            let status = sink.push(Event {
                window: WindowId::NONE,
                event: WindowEvent::YieldPoint {
                    frame_number: self.frame,
                    timestamp_ns,
                },
            });
            // Refused either way (full, or the poll was captured), so the yield point is lost.
            if status != EventStatus::Accepted {
                sink.record_drop();
            }
        }
        self.frame += 1;
        if self.exit.load(Ordering::Acquire) {
            PollStatus::Exit
        } else {
            status
        }
    }
}
