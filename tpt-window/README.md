# tpt-window

Engine-first windowing, input and surface management for Rust.

`tpt-window` is the crate most users should depend on. It re-exports
[`tpt-window-core`](../tpt-window-core) and selects the platform backend for the
target at compile time, so the same code runs on Windows, macOS, Linux (X11 and
Wayland) and the web (`wasm32`).

It is the windowing layer of the TPT AV Stack (tpt-nova, tpt-appfront, tpt-shell,
tpt-archon), but it has no dependency on those projects.

## Design

- **You own the main loop.** There is no `run(closure)` that takes over your
  `main`. You call `poll_events` (or `step`) once per frame and drain the events.
- **No allocation on the event path.** Events go into a queue you allocate up
  front with `EventQueue::with_capacity`. A full queue returns
  `EventStatus::BufferFull` and counts the drop; it never grows.
- **Window state is data.** Size, scale factor, position and focus are cached
  snapshots, so reading them does not call into the OS. Setters are queued and
  applied on the next poll, so they are safe to call from any thread.
- **Explicit AI-agent yield points.** Each step produces a `YieldPoint`, so a
  caller can run local inference between frames without losing display sync.
- **Permissive dependencies only.** The tree is checked by `cargo-deny` in CI.

## Installation

The crates are not on crates.io yet, so depend on the repository directly:

```toml
[dependencies]
tpt-window = { git = "https://github.com/tpt-solutions/tpt-window" }
```

Minimum supported Rust version: **1.85** (edition 2024).

## Quick start

```rust,no_run
use tpt_window::*;

fn main() {
    let mut event_loop = new_event_loop().expect("no windowing backend");
    let _window = WindowBuilder::new()
        .with_title("hello")
        .build(&event_loop)
        .expect("failed to create window");

    let mut queue = EventQueue::with_capacity(DEFAULT_CAPACITY);

    'main: loop {
        // You own the loop: poll, drain, update, render.
        let status = event_loop.poll_events(&mut EventSink::new(&mut queue));

        for ev in queue.drain() {
            if ev.event == WindowEvent::CloseRequested {
                break 'main;
            }
        }

        if status == PollStatus::Exit {
            break;
        }
    }
}
```

The examples in `examples/` show the same pattern:

```sh
cargo run -p tpt-window --example hello_window
cargo run -p tpt-window --example event_dump
```

## Platform support

| Target                       | Backend crate              | Status                                                                      |
|------------------------------|----------------------------|-----------------------------------------------------------------------------|
| Windows (x86_64, i686)       | `tpt-window-sys-win32`     | Implemented and tested. Touch and raw mouse are tested; pen and transparency are not. |
| macOS                        | `tpt-window-sys-cocoa`     | Implemented; CI test written, not yet seen passing.                          |
| Linux, X11                   | `tpt-window-sys-x11`       | Implemented; lifecycle test runs under Xvfb in CI.                           |
| Linux, Wayland               | `tpt-window-sys-wayland`   | Implemented; test runs under headless weston in CI. Not tried on a real compositor. |
| Web (`wasm32-unknown-unknown`) | `tpt-window-sys-wasm`    | Rendering checked in Edge with WebGPU; input and resize not yet checked in a browser. |
| Anything else                | none                       | `new_event_loop()` returns `WindowError::Unsupported`.                       |

On Linux, `new_event_loop()` uses Wayland when `WAYLAND_DISPLAY` is set and the
Wayland connection succeeds. Otherwise it uses X11.

Input IME composition (`ImeEvent::Preedit`) is implemented on Win32 only. The
other backends deliver committed text through `ImeEvent::Commit`.

## Feature flags

This facade has no feature flags of its own. Engine-facing resource types
(`WindowResource`, `InputEventQueue`) are behind the `ecs` feature of
`tpt-window-core`, which you can enable through that crate directly.

## Crates in this workspace

| Crate                       | Purpose                                                             |
|-----------------------------|---------------------------------------------------------------------|
| `tpt-window`                | This facade: re-exports core and picks the backend.                 |
| `tpt-window-core`           | Platform-agnostic types, traits and the event queue.                |
| `tpt-window-gamepad`        | Optional gamepad input over `gilrs`. Not part of the windowing layer. |
| `tpt-window-sys-*`          | Platform backends. Normally used through this facade.               |
| `tpt-window-test-utils`     | Headless backend for tests, CI and fuzzing.                         |
| `tpt-window-archon`         | Capability bridge to tpt-archon. Scaffold only.                     |

## Documentation

- Rustdoc: `cargo doc --workspace --open`
- Design specification: [spec.txt](../spec.txt)
- Implementation status and open decisions: [todo.md](../todo.md)
- Changes: [CHANGELOG.md](CHANGELOG.md)

## Scope

`tpt-window` is not a GUI toolkit (no widgets, layout or theming), not a
compositor, and not a game framework. It is also not a general-purpose
replacement for winit; it is aimed at engine and compositor workloads.

## Contributing

This project accepts issues only. Please open an issue for bugs, feature
requests and questions at <https://github.com/tpt-solutions/tpt-window/issues>.
See [CONTRIBUTING.md](../CONTRIBUTING.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
