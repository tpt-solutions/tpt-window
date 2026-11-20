# tpt-window-bevy

Bevy ECS integration for [tpt-window](../tpt-window): the window, its input queue and the event
loop as resources, and a system that pumps the loop once per frame.

```rust,ignore
let mut world = World::new();
install(&mut world, WindowBuilder::new().with_title("app"))?;
let mut schedule = Schedule::default();
schedule.add_systems(pump_system);
schedule.run(&mut world); // once per frame, on the thread that created the loop
```

The event loop is a non-send resource, so the schedule must run on that thread.

## Testing

```sh
cargo test -p tpt-window-bevy
```

The test uses the headless backend and needs no display.

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your option.
