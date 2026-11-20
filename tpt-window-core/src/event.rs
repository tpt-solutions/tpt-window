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
