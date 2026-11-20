// SPDX-License-Identifier: MIT OR Apache-2.0
//! Headless backend for CI, fuzzing, examples and replays: no OS window, scripted events.
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
//!
//! The harness also schedules events for a frame, captures the commands the application sends,
//! scripts the display list, and replays a recording:
//!
//! ```
//! use tpt_window_core::*;
//! use tpt_window_test_utils::{assert_events, headless};
//!
//! let (mut event_loop, handle) = headless();
//! let window = WindowBuilder::new().build(&event_loop).unwrap();
//! handle.inject_at(1, window.id(), WindowEvent::Focused(true));
//!
//! let mut queue = EventQueue::with_capacity(16);
//! event_loop.set_emit_yield_points(false);
//! event_loop.poll_events(&mut EventSink::new(&mut queue)); // frame 0: nothing yet
//! assert_events!(queue, []);
//! event_loop.poll_events(&mut EventSink::new(&mut queue)); // frame 1
//! assert_events!(queue, [WindowEvent::Focused(true)]);
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

use tpt_window_core::record::{LogError, read_log};
use tpt_window_core::{
    Event, EventLoop, EventLoopBackend, EventSink, EventStatus, MonitorInfo, PollStatus,
    RawDisplayHandle, RawWindowHandle, SharedWindowState, WindowAttributes, WindowBackend,
    WindowCommand, WindowError, WindowEvent, WindowId, WindowState,
};

#[derive(Default)]
struct Shared {
    queue: Mutex<Queues>,
    wake: Condvar,
    exit: AtomicBool,
    next_window: AtomicU64,
    /// Drains so far. The next drain is frame `frame`.
    frame: AtomicU64,
}

#[derive(Default)]
struct Queues {
    events: VecDeque<Event>,
    commands: VecDeque<(WindowId, Arc<SharedWindowState>, WindowCommand)>,
    /// Events held back until their frame: `(frame, event)`.
    scheduled: Vec<(u64, Event)>,
    /// Every command applied, in order, for tests to inspect.
    captured: Vec<(WindowId, WindowCommand)>,
    monitors: Vec<MonitorInfo>,
    /// Ask the loop to exit once this frame is reached.
    exit_at: Option<u64>,
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

/// Rebuilds a recording as a headless loop: each recorded frame's events are delivered on the
/// matching poll, and the loop exits after the last frame. Windows must be created in the same
/// order as in the recording, so they get the same ids.
pub fn replay(log: &[u8]) -> Result<(EventLoop, HeadlessHandle), LogError> {
    let events = read_log(log)?;
    let (el, handle) = headless();
    // Group the events into frames. A yield point ends a frame; it is not replayed, because the
    // loop emits its own.
    let mut frame = 0_u64;
    let mut any_in_frame = false;
    for ev in events {
        if matches!(ev.event, WindowEvent::YieldPoint { .. }) {
            frame += 1;
            any_in_frame = false;
            continue;
        }
        handle.inject_at(frame, ev.window, ev.event);
        any_in_frame = true;
    }
    if any_in_frame {
        frame += 1;
    }
    handle.exit_after(frame);
    Ok((el, handle))
}

/// Injects scripted OS events into a headless loop and inspects what the loop did.
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

    /// Queues an event for `window` to be delivered on frame `frame`. Frame 0 is the first poll.
    /// An event for a frame that has already passed is delivered on the next poll.
    pub fn inject_at(&self, frame: u64, window: WindowId, event: WindowEvent) {
        let mut q = lock(&self.shared.queue);
        q.scheduled.push((frame, Event { window, event }));
        drop(q);
        self.shared.wake.notify_all();
    }

    /// The frame the next poll will deliver. It is the number of polls made so far.
    pub fn frame(&self) -> u64 {
        self.shared.frame.load(Ordering::Acquire)
    }

    /// Makes the loop report `PollStatus::Exit` once frame `frame` is reached.
    pub fn exit_after(&self, frame: u64) {
        lock(&self.shared.queue).exit_at = Some(frame);
        self.shared.wake.notify_all();
    }

    /// Replaces the display list the loop reports.
    pub fn set_monitors(&self, monitors: Vec<MonitorInfo>) {
        lock(&self.shared.queue).monitors = monitors;
    }

    /// Every command applied so far, in order, with the window it was sent to.
    pub fn captured_commands(&self) -> Vec<(WindowId, WindowCommand)> {
        lock(&self.shared.queue).captured.clone()
    }

    /// Pre-reserves queue space so later `inject` calls do not allocate.
    pub fn reserve(&self, additional: usize) {
        lock(&self.shared.queue).events.reserve(additional);
    }
}

/// Checks the events in a queue, in order, against patterns, and drains the queue.
///
/// ```
/// use tpt_window_core::*;
/// use tpt_window_test_utils::assert_events;
///
/// let mut q = EventQueue::with_capacity(4);
/// q.push(Event { window: WindowId(1), event: WindowEvent::Focused(false) });
/// assert_events!(q, [WindowEvent::Focused(false)]);
/// ```
#[macro_export]
macro_rules! assert_events {
    ($queue:expr, [ $($pat:pat),* $(,)? ]) => {{
        let got: ::std::vec::Vec<$crate::__WindowEvent> =
            $queue.drain().map(|e| e.event).collect();
        let mut index = 0_usize;
        $(
            assert!(
                index < got.len(),
                "expected event {} ({}), got only {:?}",
                index,
                stringify!($pat),
                got
            );
            assert!(
                ::std::matches!(got[index], $pat),
                "event {} should match {}, got {:?}",
                index,
                stringify!($pat),
                got[index]
            );
            index += 1;
        )*
        assert_eq!(
            got.len(),
            index,
            "unexpected extra events after {}: {:?}",
            index,
            &got[index..]
        );
    }};
}

#[doc(hidden)]
pub use tpt_window_core::WindowEvent as __WindowEvent;

/// Scenario helpers for the cases apps most often get wrong.
pub mod scenarios {
    use tpt_window_core::{
        Event, MonitorInfo, PhysicalPosition, PhysicalSize, WindowEvent, WindowId,
    };

    use super::HeadlessHandle;

    /// Schedules a DPI change on `frame`: the window moves to `scale_factor`, and its inner size
    /// is `new_inner_size` in physical pixels, as the platform reports it.
    pub fn dpi_change(
        handle: &HeadlessHandle,
        frame: u64,
        window: WindowId,
        scale_factor: f64,
        new_inner_size: PhysicalSize<u32>,
    ) {
        handle.inject_at(
            frame,
            window,
            WindowEvent::ScaleFactorChanged {
                scale_factor,
                new_inner_size,
            },
        );
    }

    /// Two displays side by side: a 1920x1080 primary at the origin and a 2560x1440 display at
    /// 1920 pixels to its right, at 1.5 times the scale.
    pub fn two_monitors() -> Vec<MonitorInfo> {
        vec![
            MonitorInfo {
                name: Some("primary".into()),
                position: PhysicalPosition::new(0, 0),
                size: PhysicalSize::new(1920, 1080),
                scale_factor: 1.0,
                primary: true,
            },
            MonitorInfo {
                name: Some("secondary".into()),
                position: PhysicalPosition::new(1920, 0),
                size: PhysicalSize::new(2560, 1440),
                scale_factor: 1.5,
                primary: false,
            },
        ]
    }

    /// Schedules the window moving from its display to `to` on `frame`, which the platform
    /// reports as a scale change followed by a move.
    pub fn move_to_display(
        handle: &HeadlessHandle,
        frame: u64,
        window: WindowId,
        to: &MonitorInfo,
    ) {
        handle.inject_at(
            frame,
            window,
            WindowEvent::ScaleFactorChanged {
                scale_factor: to.scale_factor,
                new_inner_size: to.size,
            },
        );
        handle.inject_at(frame, window, WindowEvent::Moved(to.position));
    }

    /// Wraps an event for `window`, for building scripts by hand.
    pub fn event(window: WindowId, event: WindowEvent) -> Event {
        Event { window, event }
    }
}

/// Strategies that generate window events, for property tests. Enable the `proptest` feature.
#[cfg(feature = "proptest")]
pub mod strategies {
    use proptest::prelude::*;
    use tpt_window_core::{
        ElementState, GamepadId, ImeEvent, ImeText, KeyCode, MouseButton, PhysicalKey,
        PhysicalPosition, PhysicalSize, WindowEvent,
    };

    /// Any `KeyCode` the crate knows about.
    pub fn any_key() -> impl Strategy<Value = KeyCode> {
        (0_u32..125).prop_map(|tag| {
            tpt_window_core::codec::key_from_tag(tag).expect("every tag below 125 has a key")
        })
    }

    /// Any window event a backend can report, with plausible field values.
    pub fn any_window_event() -> impl Strategy<Value = WindowEvent> {
        prop_oneof![
            Just(WindowEvent::CloseRequested),
            Just(WindowEvent::CursorEntered),
            Just(WindowEvent::CursorLeft),
            Just(WindowEvent::RedrawRequested),
            any::<bool>().prop_map(WindowEvent::Focused),
            any::<bool>().prop_map(WindowEvent::Occluded),
            (any::<u32>(), any::<u32>())
                .prop_map(|(w, h)| WindowEvent::Resized(PhysicalSize::new(w, h))),
            (any::<i32>(), any::<i32>())
                .prop_map(|(x, y)| WindowEvent::Moved(PhysicalPosition::new(x, y))),
            (any::<bool>(), any_key()).prop_map(|(pressed, key)| WindowEvent::KeyboardInput {
                state: if pressed {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                },
                key_code: key,
                physical_key: PhysicalKey::Code(key),
            }),
            (any::<i32>(), any::<i32>())
                .prop_map(|(dx, dy)| WindowEvent::RawMouseMotion { dx, dy }),
            (any::<u8>(), any::<bool>()).prop_map(|(b, pressed)| WindowEvent::MouseInput {
                state: if pressed {
                    ElementState::Pressed
                } else {
                    ElementState::Released
                },
                button: MouseButton::Other(u16::from(b)),
            }),
            any::<u32>().prop_map(|id| WindowEvent::GamepadConnected(GamepadId(id))),
            "[a-zA-Z0-9 ]{0,24}".prop_map(|s| WindowEvent::Ime(ImeEvent::Commit {
                text: ImeText::new(&s),
                continued: false,
            })),
            any::<u64>().prop_map(WindowEvent::User),
        ]
    }
}

struct HeadlessBackend {
    shared: Arc<Shared>,
}

impl HeadlessBackend {
    fn apply_commands(q: &mut Queues) {
        while let Some((id, state, cmd)) = q.commands.pop_front() {
            q.captured.push((id, cmd.clone()));
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
                _ => None, // the rest has no headless effect; the capture still records it
            };
            if let Some(event) = event {
                q.events.push_back(Event { window: id, event });
            }
        }
    }

    /// Moves the events scheduled for this frame or earlier into the ready queue, in schedule
    /// order.
    fn release_scheduled(q: &mut Queues, frame: u64) {
        let mut i = 0;
        while i < q.scheduled.len() {
            if q.scheduled[i].0 <= frame {
                let (_, ev) = q.scheduled.remove(i);
                q.events.push_back(ev);
            } else {
                i += 1;
            }
        }
    }

    fn drain_into(&self, sink: &mut EventSink<'_>) -> PollStatus {
        let frame = self.shared.frame.fetch_add(1, Ordering::AcqRel);
        let mut q = lock(&self.shared.queue);
        Self::apply_commands(&mut q);
        Self::release_scheduled(&mut q, frame);
        if q.exit_at.is_some_and(|at| frame >= at) {
            self.shared.exit.store(true, Ordering::Release);
        }
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

    fn monitors(&self) -> Vec<MonitorInfo> {
        lock(&self.shared.queue).monitors.clone()
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
    fn current_monitor(&self) -> Option<MonitorInfo> {
        lock(&self.shared.queue).monitors.first().cloned()
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

    #[test]
    fn scheduled_events_arrive_on_their_frame() {
        let (mut el, h) = headless();
        el.set_emit_yield_points(false);
        let w = WindowBuilder::new().build(&el).unwrap();
        h.inject_at(2, w.id(), WindowEvent::CursorLeft);
        let mut q = EventQueue::with_capacity(4);
        assert_eq!(h.frame(), 0);
        let (_, first) = poll(&mut el, &mut q);
        let (_, second) = poll(&mut el, &mut q);
        assert!(
            first.is_empty() && second.is_empty(),
            "frames 0 and 1 hold nothing"
        );
        let (_, third) = poll(&mut el, &mut q);
        assert_eq!(third.len(), 1);
        assert_eq!(third[0].event, WindowEvent::CursorLeft);
        assert_eq!(h.frame(), 3);
    }

    #[test]
    fn commands_are_captured_in_order() {
        let (mut el, h) = headless();
        el.set_emit_yield_points(false);
        let w = WindowBuilder::new().build(&el).unwrap();
        w.set_title("first");
        w.set_visible(false);
        let mut q = EventQueue::with_capacity(4);
        poll(&mut el, &mut q);
        let captured = h.captured_commands();
        assert_eq!(captured.len(), 2);
        assert_eq!(captured[0].0, w.id());
        assert_eq!(captured[0].1, WindowCommand::SetTitle("first".into()));
        assert_eq!(captured[1].1, WindowCommand::SetVisible(false));
    }

    #[test]
    fn the_display_list_is_scripted() {
        let (el, h) = headless();
        assert!(el.available_monitors().is_empty());
        let m = MonitorInfo {
            name: Some("test".into()),
            position: tpt_window_core::PhysicalPosition::new(0, 0),
            size: PhysicalSize::new(1920, 1080),
            scale_factor: 1.0,
            primary: true,
        };
        h.set_monitors(vec![m.clone()]);
        assert_eq!(el.available_monitors(), vec![m]);
    }

    #[test]
    fn a_recording_replays_frame_for_frame() {
        use tpt_window_core::record::Recorder;
        // Record a short session: one event per frame, with the loop's yield points.
        let (mut el, h) = headless();
        let w = WindowBuilder::new().build(&el).unwrap();
        let mut recorder = Recorder::new();
        let mut q = EventQueue::with_capacity(16);
        h.inject(w.id(), WindowEvent::Focused(true));
        el.poll_events(&mut EventSink::new(&mut q));
        recorder.record_queue(&mut q);
        h.inject(w.id(), WindowEvent::CursorEntered);
        el.poll_events(&mut EventSink::new(&mut q));
        recorder.record_queue(&mut q);

        // Replay it and check the same events come out, frame for frame, then the loop exits.
        let (mut replayed, _rh) = replay(recorder.as_bytes()).expect("replays");
        let mut out = EventQueue::with_capacity(16);
        let mut seen = Vec::new();
        loop {
            let status = replayed.poll_events(&mut EventSink::new(&mut out));
            seen.extend(out.drain().map(|e| e.event));
            if status == PollStatus::Exit {
                break;
            }
        }
        let user: Vec<WindowEvent> = seen
            .into_iter()
            .filter(|e| !matches!(e, WindowEvent::YieldPoint { .. }))
            .collect();
        assert_eq!(
            user,
            vec![WindowEvent::Focused(true), WindowEvent::CursorEntered]
        );
    }
}
