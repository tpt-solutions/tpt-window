# tpt-window-core

The platform-agnostic core of [tpt-window](../tpt-window): event and window types,
the traits that every backend implements, and a pre-allocated event queue.

Most applications should depend on [`tpt-window`](../tpt-window) instead, which
re-exports everything in this crate and adds the platform backend. Depend on
`tpt-window-core` directly when you are writing a backend, a test double, or an
engine integration that must not pull in a platform backend.

## What is in here

| Module          | Contents                                                                        |
|-----------------|---------------------------------------------------------------------------------|
| `event`         | `Event`, `WindowEvent`. All events are `Copy`.                                  |
| `event_loop`    | `EventLoop`, the per-thread pump, and the `YieldPoint` emitted once per step.   |
| `window`        | `Window` (cached state plus queued setters) and `WindowBuilder`.                |
| `backend`       | `EventLoopBackend` and `WindowBackend` traits, `WindowAttributes`, `PollStatus`. |
| `sink`          | `EventQueue` (caller-owned storage) and `EventSink` (the write side).           |
| `ring`          | `RingBuffer` and `OverflowPolicy`. `DEFAULT_CAPACITY` is the default size.      |
| `input`         | `KeyCode`, `PhysicalKey`, `ElementState`, `MouseButton`, `MouseScrollDelta`, `TouchEvent`, `TouchPhase`, `GamepadId`. |
| `text`          | `ImeEvent` (`Preedit`, `Commit`), plus fixed-size inline text buffers.          |
| `dpi`           | `PhysicalSize`, `LogicalSize`, `PhysicalPosition`, `LogicalPosition`.           |
| `security`      | `CapabilityToken`, `WindowScope`, `InputScope`, `DisplayScope`, `SurfaceHandle`. |
| `protocol`      | Wire-level types for the display-server protocol (in progress).                 |
| `ids`           | `WindowId`.                                                                     |
| `error`         | `WindowError`.                                                                  |
| `ecs` (feature) | `WindowResource` and `InputEventQueue` for ECS engines.                         |

## Event loop contract

```rust,ignore
use tpt_window_core::*;

let mut queue = EventQueue::with_capacity(1024);
loop {
    let mut sink = EventSink::new(&mut queue);
    if event_loop.poll_events(&mut sink) == PollStatus::Exit {
        break;
    }
    for ev in queue.drain() {
        // handle ev.event
    }
}
```

- `poll_events` never blocks. `step(timeout)` blocks until an event arrives or the
  timeout expires.
- Events are written into the queue you pass in. The core never allocates on that
  path.
- When the queue is full, `EventStatus::BufferFull` is returned and a drop counter
  is incremented. Whether to drop the newest or oldest is a consumer policy
  (`OverflowPolicy`).
- Each step emits a `YieldPoint` (numbered, with a drop count). A missed yield shows
  up as a gap in the frame numbers.

## Writing a backend

A backend implements two traits:

- `EventLoopBackend` drives the OS message pump and writes events into the sink.
- `WindowBackend` owns one native window. It holds a `SharedWindowState` snapshot
  that the public `Window` reads without a syscall. Setters are sent as
  `WindowCommand` values and applied on the loop thread during the next poll.

`WindowBackend` requires `Send + Sync`. The `RawWindowHandle` and
`RawDisplayHandle` re-exported from `raw-window-handle` let a renderer such as
wgpu create a surface.

## Features

| Feature | Default | Effect                                                        |
|---------|---------|---------------------------------------------------------------|
| `ecs`   | off     | Adds `ecs::WindowResource` and `ecs::InputEventQueue`.         |

## Platform notes

- On `wasm32`, `Instant` is replaced by `web-time`, because `std::time::Instant`
  panics in the browser.
- The crate uses `#![forbid(unsafe_code)]`. All `unsafe` FFI lives in the
  backends.
- `#![warn(missing_docs)]` is enabled. Rustdoc coverage for every public item is
  still in progress (see [todo.md](../todo.md), Phase 5).

## Testing

```sh
cargo test -p tpt-window-core
cargo test -p tpt-window-core --features ecs
```

The tests cover ring-buffer and event-queue behaviour under property tests
(`tests/properties.rs`) and an allocation-counting check that the event hot path makes no heap
allocations (`tests/rt_safety.rs`).

## Documentation

- Rustdoc: `cargo doc -p tpt-window-core --open`
- Changes: [CHANGELOG.md](CHANGELOG.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
