// SPDX-License-Identifier: MIT OR Apache-2.0
//! Headless backend for CI, fuzzing and examples: no OS window, scripted events.
//!
//! ```
//! use tpt_window_core::*;
//! use tpt_window_test_utils::headless;
//!
//! let (mut event_loop, handle) = headless();
//! let window = WindowBuilder::new().build(&event_loop).unwrap();
//! handle.inject(window.id(), WindowEvent::CloseRequested);
//!
//! let mut queue = EventQueue::with_capacity(16);
//! event_loop.poll_events(&mut EventSink::new(&mut queue));
//! assert_eq!(queue.pop().unwrap().event, WindowEvent::CloseRequested);
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use tpt_window_core::{
    Event, EventLoop, EventLoopBackend, EventSink, EventStatus, PollStatus, RawDisplayHandle,
    RawWindowHandle, SharedWindowState, WindowAttributes, WindowBackend, WindowCommand,
    WindowError, WindowEvent, WindowId, WindowState,
};

#[derive(Default)]
struct Shared {
    queue: Mutex<Queues>,
    wake: Condvar,
    exit: std::sync::atomic::AtomicBool,
    next_window: AtomicU64,
}

#[derive(Default)]
struct Queues {
    events: VecDeque<Event>,
    commands: VecDeque<(WindowId, Arc<SharedWindowState>, WindowCommand)>,
}

fn lock(m: &Mutex<Queues>) -> MutexGuard<'_, Queues> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Creates a headless event loop and a handle for injecting events.
pub fn headless() -> (EventLoop, HeadlessHandle) {
    let shared = Arc::new(Shared::default());
    let backend = HeadlessBackend {
        shared: shared.clone(),
    };
    (
        EventLoop::with_backend(Box::new(backend)),
        HeadlessHandle { shared },
    )
}

/// Injects scripted OS events into a headless loop.
#[derive(Clone)]
pub struct HeadlessHandle {
    shared: Arc<Shared>,
}

impl HeadlessHandle {
    /// Queues an event for `window`, delivered on the next poll.
    pub fn inject(&self, window: WindowId, event: WindowEvent) {
        lock(&self.shared.queue)
            .events
            .push_back(Event { window, event });
        self.shared.wake.notify_all();
    }

    /// Pre-reserves queue space so later `inject` calls do not allocate.
    pub fn reserve(&self, additional: usize) {
        lock(&self.shared.queue).events.reserve(additional);
    }
}

struct HeadlessBackend {
    shared: Arc<Shared>,
}

impl HeadlessBackend {
    fn apply_commands(q: &mut Queues) {
        while let Some((id, state, cmd)) = q.commands.pop_front() {
            let event = match cmd {
                WindowCommand::SetSize(size) => {
                    state.update(|s| s.size = size);
                    Some(WindowEvent::Resized(size))
                }
                WindowCommand::SetPosition(pos) => {
                    state.update(|s| s.position = pos);
                    Some(WindowEvent::Moved(pos))
                }
                WindowCommand::SetVisible(v) => {
                    state.update(|s| s.visible = v);
                    None
                }
                WindowCommand::RequestRedraw => Some(WindowEvent::RedrawRequested),
                _ => None, // title etc.: nothing to show headlessly
            };
            if let Some(event) = event {
                q.events.push_back(Event { window: id, event });
            }
        }
    }

    fn drain_into(&self, sink: &mut EventSink<'_>) -> PollStatus {
        let mut q = lock(&self.shared.queue);
        Self::apply_commands(&mut q);
        let mut delivered = false;
        while let Some(ev) = q.events.front().copied() {
            match sink.push(ev) {
                EventStatus::Accepted => {
                    q.events.pop_front();
                    delivered = true;
                }
                // Keep it for the next poll.
                EventStatus::BufferFull | EventStatus::Captured => break,
            }
        }
        if self.shared.exit.load(Ordering::Acquire) {
            PollStatus::Exit
        } else if delivered {
            PollStatus::Events
        } else {
            PollStatus::Idle
        }
    }
}

impl EventLoopBackend for HeadlessBackend {
    fn poll(&mut self, sink: &mut EventSink<'_>) -> PollStatus {
        self.drain_into(sink)
    }

    fn wait(&mut self, sink: &mut EventSink<'_>, timeout: Duration) -> PollStatus {
        {
            let q = lock(&self.shared.queue);
            if q.events.is_empty() && q.commands.is_empty() {
                let _ = self
                    .shared
                    .wake
                    .wait_timeout_while(q, timeout, |q| {
                        q.events.is_empty()
                            && q.commands.is_empty()
                            && !self.shared.exit.load(Ordering::Acquire)
                    })
                    .unwrap_or_else(|e| e.into_inner());
            }
        }
        self.drain_into(sink)
    }

    fn request_exit(&self) {
        self.shared.exit.store(true, Ordering::Release);
        self.shared.wake.notify_all();
    }

    fn create_window(
        &self,
        attrs: &WindowAttributes,
    ) -> Result<Arc<dyn WindowBackend>, WindowError> {
        let id = WindowId(self.shared.next_window.fetch_add(1, Ordering::Relaxed) + 1);
        let mut state = WindowState {
            visible: attrs.visible,
            ..WindowState::default()
        };
        if let Some(size) = attrs.inner_size {
            state.size = size.to_physical(state.scale_factor);
        }
        if let Some(pos) = attrs.position {
            state.position = pos.to_physical(state.scale_factor);
        }
        Ok(Arc::new(HeadlessWindow {
            id,
            state: SharedWindowState::new(state),
            shared: self.shared.clone(),
        }))
    }
}

struct HeadlessWindow {
    id: WindowId,
    state: Arc<SharedWindowState>,
    shared: Arc<Shared>,
}

impl WindowBackend for HeadlessWindow {
    fn id(&self) -> WindowId {
        self.id
    }
    fn state(&self) -> &Arc<SharedWindowState> {
        &self.state
    }
    fn raw_window_handle(&self) -> Result<RawWindowHandle, WindowError> {
        Err(WindowError::NoNativeHandle)
    }
    fn raw_display_handle(&self) -> Result<RawDisplayHandle, WindowError> {
        Err(WindowError::NoNativeHandle)
    }
    fn send_command(&self, command: WindowCommand) {
        lock(&self.shared.queue)
            .commands
            .push_back((self.id, self.state.clone(), command));
        self.shared.wake.notify_all();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_window_core::{EventQueue, PhysicalSize, WindowBuilder};

    fn poll(el: &mut EventLoop, q: &mut EventQueue) -> (PollStatus, Vec<Event>) {
        let s = el.poll_events(&mut EventSink::new(q));
        (s, q.drain().collect())
    }

    #[test]
    fn yield_point_once_per_frame_after_events() {
        let (mut el, h) = headless();
        let w = WindowBuilder::new().build(&el).unwrap();
        h.inject(w.id(), WindowEvent::Focused(true));
        let mut q = EventQueue::with_capacity(8);
        let (_, evs) = poll(&mut el, &mut q);
        assert_eq!(evs.len(), 2);
        assert_eq!(evs[0].event, WindowEvent::Focused(true));
        assert!(matches!(
            evs[1].event,
            WindowEvent::YieldPoint {
                frame_number: 0,
                ..
            }
        ));
        let (_, evs) = poll(&mut el, &mut q);
        assert!(matches!(
            evs[0].event,
            WindowEvent::YieldPoint {
                frame_number: 1,
                ..
            }
        ));
    }

    #[test]
    fn full_buffer_keeps_events_for_next_poll() {
        let (mut el, h) = headless();
        el.set_emit_yield_points(false);
        let w = WindowBuilder::new().build(&el).unwrap();
        for _ in 0..5 {
            h.inject(w.id(), WindowEvent::CursorEntered);
        }
        let mut q = EventQueue::with_capacity(2);
        let mut total = 0;
        for _ in 0..3 {
            total += poll(&mut el, &mut q).1.len();
        }
        assert_eq!(total, 5);
        assert_eq!(q.dropped(), 0);
    }

    #[test]
    fn setters_apply_on_next_poll() {
        let (mut el, _h) = headless();
        el.set_emit_yield_points(false);
        let w = WindowBuilder::new().build(&el).unwrap();
        w.set_size(PhysicalSize::new(100, 50));
        assert_eq!(w.size(), PhysicalSize::new(800, 600)); // not yet
        let mut q = EventQueue::with_capacity(4);
        let (_, evs) = poll(&mut el, &mut q);
        assert_eq!(w.size(), PhysicalSize::new(100, 50));
        assert_eq!(
            evs[0].event,
            WindowEvent::Resized(PhysicalSize::new(100, 50))
        );
    }

    #[test]
    fn exit_and_multi_window() {
        let (mut el, _h) = headless();
        let a = WindowBuilder::new().build(&el).unwrap();
        let b = WindowBuilder::new().build(&el).unwrap();
        assert_ne!(a.id(), b.id());
        el.request_exit();
        let mut q = EventQueue::with_capacity(4);
        assert_eq!(poll(&mut el, &mut q).0, PollStatus::Exit);
    }

    #[test]
    fn step_times_out() {
        let (mut el, _h) = headless();
        el.set_emit_yield_points(false);
        let mut q = EventQueue::with_capacity(4);
        let s = el.step(&mut EventSink::new(&mut q), Duration::from_millis(5));
        assert_eq!(s, PollStatus::Idle);
    }
}
