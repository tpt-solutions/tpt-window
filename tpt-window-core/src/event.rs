// SPDX-License-Identifier: MIT OR Apache-2.0
//! The event type. Strictly `Copy` so queues never allocate or run destructors.

use crate::dpi::{PhysicalPosition, PhysicalSize};
use crate::ids::WindowId;
use crate::input::*;
use crate::text::ImeEvent;

/// Everything a window can report.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowEvent {
    // Lifecycle
    /// User asked to close the window.
    CloseRequested,
    /// The window was destroyed.
    Destroyed,

    // Geometry
    /// Inner size changed (physical pixels).
    Resized(PhysicalSize<u32>),
    /// Window moved.
    Moved(PhysicalPosition<i32>),
    /// Scale factor changed.
    ScaleFactorChanged {
        /// New scale factor.
        scale_factor: f64,
        /// New inner size in physical pixels.
        new_inner_size: PhysicalSize<u32>,
    },

    // Focus & visibility
    /// Focus gained (`true`) or lost.
    Focused(bool),
    /// Window fully covered (`true`) or uncovered.
    Occluded(bool),

    // Keyboard
    /// Key press/release.
    KeyboardInput {
        /// Pressed or released.
        state: ElementState,
        /// Layout-translated key.
        key_code: KeyCode,
        /// Layout-independent key position.
        physical_key: PhysicalKey,
    },
    /// A file is being dragged over the window. A path longer than one chunk arrives as several
    /// events, with `continued` set until the last. Use [`PathAssembler`] to rebuild it.
    FileHovered {
        /// This part of the path.
        chunk: crate::text::ImeText,
        /// Whether more parts of this path follow.
        continued: bool,
    },
    /// A file was dropped on the window. Chunked like [`WindowEvent::FileHovered`].
    FileDropped {
        /// This part of the path.
        chunk: crate::text::ImeText,
        /// Whether more parts of this path follow.
        continued: bool,
    },
    /// A drag left the window without dropping anything.
    FileHoverCancelled,
    /// A value sent from another thread with [`EventLoopProxy::send_user`](crate::EventLoopProxy::send_user).
    /// Values arrive in the order they were sent.
    User(u64),
    /// The system switched between light and dark appearance.
    ThemeChanged(crate::theme::Theme),
    /// The modifier keys held changed. Sent before the key event that caused it.
    ModifiersChanged(crate::input::ModifiersState),
    /// Input method event.
    Ime(ImeEvent),

    // Pointer
    /// Cursor moved.
    CursorMoved {
        /// Position in physical pixels.
        position: PhysicalPosition<f64>,
        /// Pen/touch pressure 0.0..=1.0 (0.0 for a plain mouse).
        pressure: f32,
    },
    /// Cursor entered the window.
    CursorEntered,
    /// Cursor left the window.
    CursorLeft,
    /// Mouse button press/release.
    MouseInput {
        /// Pressed or released.
        state: ElementState,
        /// Which button.
        button: MouseButton,
    },
    /// Scroll.
    MouseWheel {
        /// Amount.
        delta: MouseScrollDelta,
        /// Gesture phase.
        phase: TouchPhase,
    },

    // Touch
    /// Touch update.
    Touch(TouchEvent),

    // Raw input
    /// Relative mouse motion from the raw input device, before pointer acceleration.
    /// Delivered whether or not the cursor moves on screen (for example in games).
    /// Only one window receives it: the window that registered for raw input.
    RawMouseMotion {
        /// Horizontal counts.
        dx: i32,
        /// Vertical counts (positive is down).
        dy: i32,
    },

    // Gamepad
    /// Gamepad connected.
    GamepadConnected(GamepadId),
    /// Gamepad disconnected.
    GamepadDisconnected(GamepadId),

    // Rendering
    /// The window should be redrawn.
    RedrawRequested,

    // AI-agent yield point
    /// Emitted once per frame after all OS events are drained: the engine may
    /// yield to an AI agent here without desynchronising display refresh.
    YieldPoint {
        /// Frame counter, starting at 0.
        frame_number: u64,
        /// Monotonic nanoseconds since the event loop was created.
        timestamp_ns: u64,
    },
}

/// A [`WindowEvent`] addressed to a window (multi-window event loops).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Event {
    /// Target window ([`WindowId::NONE`] for global events like yield points).
    pub window: WindowId,
    /// The event.
    pub event: WindowEvent,
}

// Compile-time guarantees: Copy, and small enough to keep the ring cache-friendly.
const _: () = {
    const fn assert_copy<T: Copy>() {}
    assert_copy::<WindowEvent>();
    assert_copy::<Event>();
    assert!(core::mem::size_of::<Event>() <= 64);
};

/// Rebuilds a file path from the chunks of [`WindowEvent::FileDropped`] or
/// [`WindowEvent::FileHovered`]. Feed each chunk in arrival order; when `push` returns a path,
/// the path is complete.
#[derive(Debug, Default)]
pub struct PathAssembler {
    text: String,
}

impl PathAssembler {
    /// An empty assembler.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a chunk. Returns the whole path when `continued` is false, and starts a new one
    /// afterwards.
    pub fn push(&mut self, chunk: &crate::text::ImeText, continued: bool) -> Option<String> {
        self.text.push_str(chunk.as_str());
        if continued {
            None
        } else {
            Some(std::mem::take(&mut self.text))
        }
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;
    use crate::text::ImeText;

    #[test]
    fn chunks_rebuild_a_long_path() {
        let path = "C:/Users/a very long directory name with spaces/文件 and more/file.txt";
        let mut rest = path;
        let mut asm = PathAssembler::new();
        let mut done = None;
        while !rest.is_empty() {
            let (chunk, next) = ImeText::take_chunk(rest);
            let continued = !next.is_empty();
            done = asm.push(&chunk, continued);
            rest = next;
        }
        assert_eq!(done.as_deref(), Some(path));
    }

    #[test]
    fn a_short_path_is_one_chunk() {
        let mut asm = PathAssembler::new();
        assert_eq!(
            asm.push(&ImeText::new("a.txt"), false).as_deref(),
            Some("a.txt")
        );
    }
}
