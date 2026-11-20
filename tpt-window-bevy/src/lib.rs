// SPDX-License-Identifier: MIT OR Apache-2.0
//! Bevy ECS integration for tpt-window: the window, its input queue and the event loop as
//! resources, and a system that pumps the loop once per frame.
//!
//! ```ignore
//! use bevy_ecs::prelude::*;
//! use tpt_window_bevy::{TptInput, install, pump_system};
//!
//! let mut world = World::new();
//! install(&mut world, WindowBuilder::new().with_title("app"))?;
//! let mut schedule = Schedule::default();
//! schedule.add_systems(pump_system);
//! loop {
//!     schedule.run(&mut world);
//!     for ev in world.resource_mut::<TptInput>().0.drain() { /* react */ }
//! }
//! ```
//!
//! The event loop is a non-send resource: it stays on the thread that created it, which is the
//! thread the window system requires. Run the schedule on that thread.

use bevy_ecs::prelude::*;
use tpt_window::{EventLoop, WindowBuilder, WindowError};
use tpt_window_core::{InputEventQueue, Window, WindowResource};

/// The application's window, as a resource.
#[derive(Resource)]
pub struct TptWindow(pub WindowResource);

/// The input queue the pump fills and the app reads, as a resource.
#[derive(Resource)]
pub struct TptInput(pub InputEventQueue);

/// The event loop. A non-send resource: it belongs to the thread that created it.
pub struct TptLoop(pub EventLoop);

/// Creates the event loop and a window from `builder`, and inserts them as resources.
pub fn install(world: &mut World, builder: WindowBuilder) -> Result<(), WindowError> {
    let event_loop = tpt_window::new_event_loop()?;
    let window = builder.build(&event_loop)?;
    install_with(world, event_loop, window);
    Ok(())
}

/// Inserts an event loop and a window that were already created. Use it when the loop must come
/// from elsewhere, or in tests.
pub fn install_with(world: &mut World, event_loop: EventLoop, window: Window) {
    world.insert_non_send_resource(TptLoop(event_loop));
    world.insert_resource(TptWindow(WindowResource { window }));
    world.insert_resource(TptInput(InputEventQueue::new()));
}

/// Pumps the event loop into the input queue. Add it to a schedule, and run that schedule once
/// per frame on the loop's thread.
pub fn pump_system(mut event_loop: NonSendMut<TptLoop>, mut input: ResMut<TptInput>) {
    input.0.pump(&mut event_loop.0);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tpt_window_core::WindowEvent;
    use tpt_window_test_utils::headless;

    #[test]
    fn the_pump_moves_scripted_events_into_the_resource() {
        let (event_loop, handle) = headless();
        let window = WindowBuilder::new().build(&event_loop).expect("window");
        handle.inject(window.id(), WindowEvent::Focused(true));

        let mut world = World::new();
        install_with(&mut world, event_loop, window);
        let mut schedule = Schedule::default();
        schedule.add_systems(pump_system);
        schedule.run(&mut world);

        let events: Vec<WindowEvent> = world
            .resource_mut::<TptInput>()
            .0
            .drain()
            .map(|e| e.event)
            .collect();
        assert_eq!(events.first(), Some(&WindowEvent::Focused(true)));
    }
}
