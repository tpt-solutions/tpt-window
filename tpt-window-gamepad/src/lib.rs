// SPDX-License-Identifier: MIT OR Apache-2.0
//! Optional gamepad input for tpt-window, built on `gilrs`.
//!
//! This crate is separate from `tpt-window` so the windowing layer keeps its
//! dependency footprint small. Events use tpt-window's own types, so callers never
//! depend on `gilrs`.
//!
//! ```no_run
//! use tpt_window_gamepad::{GamepadEvent, Gamepads};
//!
//! let mut pads = Gamepads::new().expect("gamepad backend");
//! let mut events = Vec::new();
//! loop {
//!     pads.poll(&mut events); // call once per frame; never blocks
//!     for ev in events.drain(..) {
//!         if let GamepadEvent::Button { button, pressed: true, .. } = ev {
//!             println!("pressed {button:?}");
//!         }
//!     }
//!     # break;
//! }
//! ```
#![warn(missing_docs)]

use std::collections::HashMap;
use std::fmt;

use gilrs::{Axis, Button, EventType, Gilrs};

pub use tpt_window_core::GamepadId;

/// A gamepad button, named after its role rather than the device's layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GamepadButton {
    /// Bottom face button (A on Xbox, Cross on PlayStation).
    South,
    /// Right face button (B / Circle).
    East,
    /// Top face button (Y / Triangle).
    North,
    /// Left face button (X / Square).
    West,
    /// Extra action button C.
    C,
    /// Extra action button Z.
    Z,
    /// Left shoulder (LB / L1).
    LeftTrigger,
    /// Left trigger (LT / L2).
    LeftTrigger2,
    /// Right shoulder (RB / R1).
    RightTrigger,
    /// Right trigger (RT / R2).
    RightTrigger2,
    /// Select / Back / Share.
    Select,
    /// Start / Menu / Options.
    Start,
    /// Mode / Guide / Home.
    Mode,
    /// Left stick press.
    LeftThumb,
    /// Right stick press.
    RightThumb,
    /// D-pad up.
    DPadUp,
    /// D-pad down.
    DPadDown,
    /// D-pad left.
    DPadLeft,
    /// D-pad right.
    DPadRight,
    /// A button the mapping does not name.
    Other,
}

/// A gamepad axis, with values in -1.0..=1.0 (triggers in 0.0..=1.0).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum GamepadAxis {
    /// Left stick, horizontal (positive is right).
    LeftStickX,
    /// Left stick, vertical (positive is up).
    LeftStickY,
    /// Left trigger as an axis.
    LeftZ,
    /// Right stick, horizontal (positive is right).
    RightStickX,
    /// Right stick, vertical (positive is up).
    RightStickY,
    /// Right trigger as an axis.
    RightZ,
    /// D-pad as an axis, horizontal.
    DPadX,
    /// D-pad as an axis, vertical.
    DPadY,
    /// An axis the mapping does not name.
    Other,
}

/// One change reported by a gamepad.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum GamepadEvent {
    /// A gamepad was plugged in.
    Connected(GamepadId),
    /// A gamepad was unplugged. Its id is not reused while the program runs.
    Disconnected(GamepadId),
    /// A button went down or up.
    Button {
        /// Which gamepad.
        id: GamepadId,
        /// Which button.
        button: GamepadButton,
        /// True when pressed.
        pressed: bool,
    },
    /// An axis moved.
    Axis {
        /// Which gamepad.
        id: GamepadId,
        /// Which axis.
        axis: GamepadAxis,
        /// Current value.
        value: f32,
    },
}

/// Error from the gamepad backend.
#[derive(Debug)]
pub struct GamepadError(String);

impl fmt::Display for GamepadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "gamepad backend: {}", self.0)
    }
}

impl std::error::Error for GamepadError {}

/// The connected gamepads. Create one and call [`Gamepads::poll`] once per frame.
pub struct Gamepads {
    gilrs: Gilrs,
    /// Stable ids handed out so far. A gilrs id is mapped to one of these the first time it is
    /// seen, and the mapping is never removed, so an id is not reused while the program runs.
    ids: HashMap<gilrs::GamepadId, GamepadId>,
    next_id: u32,
}

impl Gamepads {
    /// Starts the platform gamepad backend.
    pub fn new() -> Result<Self, GamepadError> {
        Gilrs::new()
            .map(|gilrs| Self {
                gilrs,
                ids: HashMap::new(),
                next_id: 0,
            })
            .map_err(|e| GamepadError(e.to_string()))
    }

    /// The stable id for a gilrs id, assigned on first sight.
    fn id_for(&mut self, id: gilrs::GamepadId) -> GamepadId {
        if let Some(&mapped) = self.ids.get(&id) {
            return mapped;
        }
        let mapped = GamepadId(self.next_id);
        // Running out of 32-bit ids would take 4 billion distinct pads. Stop rather than wrap
        // around and hand out an id that is already in use.
        self.next_id = self
            .next_id
            .checked_add(1)
            .expect("gamepad id space exhausted");
        self.ids.insert(id, mapped);
        mapped
    }

    /// Appends every change since the last call to `out`. Never blocks.
    pub fn poll(&mut self, out: &mut Vec<GamepadEvent>) {
        while let Some(ev) = self.gilrs.next_event() {
            let id = self.id_for(ev.id);
            match ev.event {
                EventType::Connected => out.push(GamepadEvent::Connected(id)),
                EventType::Disconnected => out.push(GamepadEvent::Disconnected(id)),
                EventType::ButtonPressed(b, _) => out.push(GamepadEvent::Button {
                    id,
                    button: map_button(b),
                    pressed: true,
                }),
                EventType::ButtonReleased(b, _) => out.push(GamepadEvent::Button {
                    id,
                    button: map_button(b),
                    pressed: false,
                }),
                EventType::AxisChanged(a, value, _) => out.push(GamepadEvent::Axis {
                    id,
                    axis: map_axis(a),
                    value,
                }),
                // Analog button values and key repeats are not part of this API.
                _ => {}
            }
        }
    }
}

fn map_button(b: Button) -> GamepadButton {
    match b {
        Button::South => GamepadButton::South,
        Button::East => GamepadButton::East,
        Button::North => GamepadButton::North,
        Button::West => GamepadButton::West,
        Button::C => GamepadButton::C,
        Button::Z => GamepadButton::Z,
        Button::LeftTrigger => GamepadButton::LeftTrigger,
        Button::LeftTrigger2 => GamepadButton::LeftTrigger2,
        Button::RightTrigger => GamepadButton::RightTrigger,
        Button::RightTrigger2 => GamepadButton::RightTrigger2,
        Button::Select => GamepadButton::Select,
        Button::Start => GamepadButton::Start,
        Button::Mode => GamepadButton::Mode,
        Button::LeftThumb => GamepadButton::LeftThumb,
        Button::RightThumb => GamepadButton::RightThumb,
        Button::DPadUp => GamepadButton::DPadUp,
        Button::DPadDown => GamepadButton::DPadDown,
        Button::DPadLeft => GamepadButton::DPadLeft,
        Button::DPadRight => GamepadButton::DPadRight,
        _ => GamepadButton::Other,
    }
}

fn map_axis(a: Axis) -> GamepadAxis {
    match a {
        Axis::LeftStickX => GamepadAxis::LeftStickX,
        Axis::LeftStickY => GamepadAxis::LeftStickY,
        Axis::LeftZ => GamepadAxis::LeftZ,
        Axis::RightStickX => GamepadAxis::RightStickX,
        Axis::RightStickY => GamepadAxis::RightStickY,
        Axis::RightZ => GamepadAxis::RightZ,
        Axis::DPadX => GamepadAxis::DPadX,
        Axis::DPadY => GamepadAxis::DPadY,
        _ => GamepadAxis::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn face_buttons_keep_their_roles() {
        assert_eq!(map_button(Button::South), GamepadButton::South);
        assert_eq!(map_button(Button::Start), GamepadButton::Start);
        assert_eq!(map_button(Button::DPadLeft), GamepadButton::DPadLeft);
    }

    #[test]
    fn unnamed_buttons_and_axes_map_to_other() {
        assert_eq!(map_button(Button::Unknown), GamepadButton::Other);
        assert_eq!(map_axis(Axis::Unknown), GamepadAxis::Other);
    }

    #[test]
    fn axes_keep_their_roles() {
        assert_eq!(map_axis(Axis::LeftStickX), GamepadAxis::LeftStickX);
        assert_eq!(map_axis(Axis::RightZ), GamepadAxis::RightZ);
    }

    #[test]
    fn backend_starts_and_polls_without_blocking() {
        // Works on machines without pads too; the event list may simply be empty.
        match Gamepads::new() {
            Ok(mut pads) => {
                let mut events = Vec::new();
                pads.poll(&mut events);
            }
            Err(e) => eprintln!("no gamepad backend here: {e}"),
        }
    }
}
