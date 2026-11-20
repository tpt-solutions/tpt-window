// SPDX-License-Identifier: MIT OR Apache-2.0
//! Platform-agnostic traits, types and the zero-allocation event queue for tpt-window.
//!
//! The caller owns the main loop and a pre-allocated [`EventQueue`]:
//!
//! ```ignore
//! let mut queue = EventQueue::with_capacity(1024);
//! loop {
//!     let mut sink = EventSink::new(&mut queue);
//!     if event_loop.poll_events(&mut sink) == PollStatus::Exit { break; }
//!     for ev in queue.drain() { /* handle */ }
//! }
//! ```
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod backend;
pub mod channel;
pub mod dpi;
#[cfg(feature = "ecs")]
pub mod ecs;
pub mod error;
pub mod event;
pub mod event_loop;
pub mod ids;
pub mod input;
pub mod protocol;
pub mod ring;
pub mod security;
pub mod sink;
pub mod text;
pub mod window;

pub use backend::{
    EventLoopBackend, PollStatus, SharedWindowState, WindowAttributes, WindowBackend,
    WindowCommand, WindowState,
};
pub use dpi::{LogicalPosition, LogicalSize, PhysicalPosition, PhysicalSize};
pub use error::WindowError;
pub use event::{Event, WindowEvent};
pub use event_loop::EventLoop;
pub use ids::WindowId;
pub use input::*;
pub use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
pub use ring::{DEFAULT_CAPACITY, OverflowPolicy, RingBuffer};
pub use security::{CapabilityToken, DisplayScope, InputScope, SurfaceHandle, WindowScope};
pub use sink::{EventQueue, EventSink, EventStatus, PENDING_CAPACITY, new_pending_queue};
pub use text::{ImeEvent, ImeText, InlineText};
pub use window::{Window, WindowBuilder};

#[cfg(feature = "ecs")]
pub use ecs::{InputEventQueue, WindowResource};
