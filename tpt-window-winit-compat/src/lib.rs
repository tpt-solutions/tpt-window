// SPDX-License-Identifier: MIT OR Apache-2.0
//! A winit 0.30 style `ApplicationHandler` over tpt-window's `step` loop.
//!
//! Applications written against winit's `run_app` can move with few changes: implement
//! [`ApplicationHandler`], create windows in [`resumed`](ApplicationHandler::resumed), and call
//! [`run_app`] instead of `EventLoop::run_app`. The windows and the event types are tpt-window's
//! own, so the handler sees [`tpt_window::WindowEvent`] and [`tpt_window::WindowId`].
//!
//! The loop is driven by tpt-window, so it runs the callbacks in this order:
//!
//! 1. [`resumed`](ApplicationHandler::resumed), once, before the first event.
//! 2. [`window_event`](ApplicationHandler::window_event), once per event, in arrival order.
//! 3. [`about_to_wait`](ApplicationHandler::about_to_wait), after each batch of events, and also
//!    when a wait times out with nothing to report (every [`IDLE_WAIT`] at most).
//! 4. [`exiting`](ApplicationHandler::exiting), once, when the loop stops.
//!
//! ```no_run
//! use tpt_window_winit_compat::{ActiveLoop, ApplicationHandler, run_app};
//! use tpt_window::{Window, WindowBuilder, WindowEvent, WindowId, WindowError};
//!
//! #[derive(Default)]
//! struct App { window: Option<Window> }
//!
//! impl ApplicationHandler for App {
//!     fn resumed(&mut self, ctx: &mut ActiveLoop) {
//!         self.window = Some(ctx.create_window(WindowBuilder::new().with_title("app")).unwrap());
//!     }
//!     fn window_event(&mut self, ctx: &mut ActiveLoop, _id: WindowId, ev: WindowEvent) {
//!         if ev == WindowEvent::CloseRequested { ctx.exit(); }
//!     }
//!     fn about_to_wait(&mut self, _ctx: &mut ActiveLoop) { /* render here */ }
//! }
//!
//! run_app(&mut App::default()).unwrap();
//! ```
#![warn(missing_docs)]

use std::time::Duration;

use tpt_window::{DEFAULT_CAPACITY, EventLoop, EventQueue, EventSink, PollStatus, Window};
pub use tpt_window::{WindowBuilder, WindowError, WindowEvent, WindowId};

/// The longest a wait lasts when nothing happens, so `about_to_wait` still runs regularly.
pub const IDLE_WAIT: Duration = Duration::from_millis(16);

/// How the loop waits between events, as in winit's `ControlFlow`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ControlFlow {
    /// Wait for events, waking at least every [`IDLE_WAIT`] so `about_to_wait` keeps running.
    #[default]
    Wait,
    /// Do not wait: run `about_to_wait` as often as the loop turns. Use it for a game loop that
    /// renders continuously.
    Poll,
    /// Wait for events, but wake no later than this instant.
    WaitUntil(std::time::Instant),
}

impl ControlFlow {
    /// How long the next `step` may wait.
    fn timeout(self, now: std::time::Instant) -> Duration {
        match self {
            ControlFlow::Wait => IDLE_WAIT,
            ControlFlow::Poll => Duration::ZERO,
            ControlFlow::WaitUntil(at) => at.saturating_duration_since(now),
        }
    }
}

/// The callbacks an application implements, mirroring winit 0.30's `ApplicationHandler`.
pub trait ApplicationHandler {
    /// Called once, before any event. Create the application's windows here.
    fn resumed(&mut self, ctx: &mut ActiveLoop<'_>);

    /// Called for each window event, in arrival order.
    fn window_event(&mut self, ctx: &mut ActiveLoop<'_>, window: WindowId, event: WindowEvent);

    /// Called for each value sent with [`ActiveLoop::proxy`]'s `send_user`, in the order sent.
    /// The default ignores them.
    fn user_event(&mut self, _ctx: &mut ActiveLoop<'_>, _value: u64) {}

    /// Called after each batch of events, and on idle ticks. Render here.
    fn about_to_wait(&mut self, _ctx: &mut ActiveLoop<'_>) {}

    /// Called once when the loop stops, after the last callback.
    fn exiting(&mut self) {}
}

/// The loop as seen from a callback: create windows and request the exit.
pub struct ActiveLoop<'a> {
    /// The wait for the next step. Set through [`ActiveLoop::set_control_flow`].
    control: ControlFlow,
    event_loop: &'a EventLoop,
    exit: bool,
}

impl ActiveLoop<'_> {
    /// Creates a window. Keep the returned [`Window`] alive: dropping it closes the window.
    pub fn create_window(&self, builder: WindowBuilder) -> Result<Window, WindowError> {
        builder.build(self.event_loop)
    }

    /// Sets how the loop waits from the next step on.
    pub fn set_control_flow(&mut self, control: ControlFlow) {
        self.control = control;
    }

    /// The current way of waiting.
    pub fn control_flow(&self) -> ControlFlow {
        self.control
    }

    /// Stops the loop after the current callback returns.
    pub fn exit(&mut self) {
        self.exit = true;
    }

    /// A handle that other threads use to wake the loop or stop it.
    pub fn proxy(&self) -> tpt_window::EventLoopProxy {
        self.event_loop.proxy()
    }
}

/// Runs `app` until it calls [`ActiveLoop::exit`], the platform asks the application to stop,
/// or the loop fails.
pub fn run_app<A: ApplicationHandler>(app: &mut A) -> Result<(), WindowError> {
    let mut event_loop = tpt_window::new_event_loop()?;
    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);
    // The way of waiting persists across callbacks, so each callback's context starts from it.
    let mut control = ControlFlow::default();

    {
        let mut ctx = ActiveLoop {
            control,
            event_loop: &event_loop,
            exit: false,
        };
        app.resumed(&mut ctx);
        control = ctx.control;
        if ctx.exit {
            app.exiting();
            return Ok(());
        }
    }

    loop {
        let timeout = control.timeout(std::time::Instant::now());
        let status = event_loop.step(&mut EventSink::new(&mut queue), timeout);
        let mut exit = status == PollStatus::Exit;
        {
            let mut ctx = ActiveLoop {
                control,
                event_loop: &event_loop,
                exit: false,
            };
            for ev in queue.drain() {
                // Yield points are tpt-window's frame boundaries; winit has no such event.
                if matches!(ev.event, WindowEvent::YieldPoint { .. }) {
                    continue;
                }
                if let WindowEvent::User(value) = ev.event {
                    app.user_event(&mut ctx, value);
                } else {
                    app.window_event(&mut ctx, ev.window, ev.event);
                }
                if ctx.exit {
                    break;
                }
            }
            if !ctx.exit {
                app.about_to_wait(&mut ctx);
            }
            control = ctx.control;
            exit |= ctx.exit;
        }
        if exit {
            if status != PollStatus::Exit {
                event_loop.request_exit();
            }
            app.exiting();
            return Ok(());
        }
    }
}
