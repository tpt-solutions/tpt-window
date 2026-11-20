// SPDX-License-Identifier: MIT OR Apache-2.0
//! A handle that other threads use to wake the event loop or ask it to stop.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// What a backend provides so that another thread can interrupt a blocked `step`.
pub trait Wake: Send + Sync {
    /// Interrupts a blocked [`EventLoop::step`](crate::EventLoop::step) so it returns soon.
    fn wake(&self);
}

/// Used by backends that cannot interrupt a wait. The loop then only notices the wake-up when
/// its `step` timeout expires, so keep that timeout short if you rely on the proxy.
pub(crate) struct NoWake;

impl Wake for NoWake {
    fn wake(&self) {}
}

/// A `Send` and `Clone` handle to an event loop, usable from any thread.
///
/// [`wake`](Self::wake) makes a blocked `step` return, and [`request_exit`](Self::request_exit)
/// makes the next poll report `PollStatus::Exit`. Use these to hand results from a worker
/// thread to the loop, or to stop the loop when a background job finishes.
#[derive(Clone)]
pub struct EventLoopProxy {
    wake: Arc<dyn Wake>,
    exit: Arc<AtomicBool>,
    user: Arc<Mutex<VecDeque<u64>>>,
}

impl EventLoopProxy {
    pub(crate) fn new(
        wake: Arc<dyn Wake>,
        exit: Arc<AtomicBool>,
        user: Arc<Mutex<VecDeque<u64>>>,
    ) -> Self {
        Self { wake, exit, user }
    }

    /// Sends `value` to the loop, which reports it as [`WindowEvent::User`](crate::WindowEvent::User),
    /// and wakes the loop. Values keep the order they were sent in.
    pub fn send_user(&self, value: u64) {
        self.user
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push_back(value);
        self.wake.wake();
    }

    /// Interrupts a blocked `step`.
    pub fn wake(&self) {
        self.wake.wake();
    }

    /// Asks the loop to stop, and wakes it so it does so promptly.
    pub fn request_exit(&self) {
        self.exit.store(true, Ordering::Release);
        self.wake.wake();
    }
}
