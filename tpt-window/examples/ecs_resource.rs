// SPDX-License-Identifier: MIT OR Apache-2.0
//! An engine keeps the window and its input queue as resources, and one system per frame pumps
//! the queue and reads it. Requires the `ecs` feature.
//!
//! `cargo run -p tpt-window --example ecs_resource --features ecs`
//!
//! Set `TPT_EXAMPLE_SECONDS=5` to auto-exit.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use tpt_window::prelude::*;
use tpt_window::{EventLoop, InputEventQueue, WindowResource};

/// A toy resource store, standing in for an ECS world.
#[derive(Default)]
struct World {
    resources: HashMap<&'static str, Box<dyn std::any::Any>>,
}

impl World {
    fn insert<T: 'static>(&mut self, name: &'static str, value: T) {
        self.resources.insert(name, Box::new(value));
    }
    fn get_mut<T: 'static>(&mut self, name: &'static str) -> Option<&mut T> {
        self.resources.get_mut(name)?.downcast_mut::<T>()
    }
}

/// The input system: pump the queue once per frame, then read what arrived.
fn input_system(world: &mut World) -> bool {
    let mut quit = false;
    // Take the queue out, so the event loop can be borrowed alongside it.
    let mut input: InputEventQueue = *world
        .resources
        .remove("input")
        .expect("input resource")
        .downcast()
        .expect("InputEventQueue");
    let event_loop: &mut EventLoop = world.get_mut("event_loop").expect("event loop");
    input.pump(event_loop);
    for ev in input.drain() {
        if ev.event == WindowEvent::CloseRequested {
            quit = true;
        }
    }
    world.insert("input", input);
    quit
}

fn main() -> Result<(), WindowError> {
    let limit = std::env::var("TPT_EXAMPLE_SECONDS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .map(Duration::from_secs_f64);
    let started = Instant::now();

    let event_loop = new_event_loop()?;
    let window = WindowBuilder::new()
        .with_title("tpt-window: ECS resources")
        .build(&event_loop)?;

    let mut world = World::default();
    world.insert("event_loop", event_loop);
    world.insert("window", WindowResource { window });
    world.insert("input", InputEventQueue::new());

    loop {
        let quit = input_system(&mut world);
        // Rendering would read the window resource here.
        let timed_out = limit.is_some_and(|l| started.elapsed() >= l);
        if quit || timed_out {
            break;
        }
    }
    Ok(())
}
