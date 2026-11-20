# tpt-window

Engine-first windowing, input and surface management for the TPT AV Stack
(tpt-nova, tpt-appfront, tpt-shell, tpt-archon). It is written in Rust and runs on
Windows, macOS, Linux (X11 and Wayland) and the web (`wasm32`).

- **You own the main loop.** There is no `run(closure)` that takes over `main`. You
  call `poll_events` (or `step`) once per frame and drain the events.
- **No allocation on the event hot path.** Events go into a queue you allocate up
  front. A full queue returns `EventStatus::BufferFull` and counts the drop.
- **Window state is data.** Size, scale factor, position and focus are cached
  snapshots, so reading them does not call the OS. Setters are queued and applied on
  the next poll, so they are safe from any thread.
- **Explicit AI-agent yield points.** Each step produces a `YieldPoint`, so local
  inference can run between frames without losing display sync.
- **Permissive dependencies only.** `cargo-deny` checks the tree in CI.

**Status:** pre-release (0.1.0), not yet on crates.io. Per-platform verification is
tracked in [todo.md](todo.md).

## Quick start

Depend on the facade crate straight from the repository:

```toml
[dependencies]
tpt-window = { git = "https://github.com/tpt-solutions/tpt-window" }
```

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

## First 10 minutes

Run the examples. Each one opens a window; set `TPT_EXAMPLE_SECONDS=5` to have it exit by itself.

```sh
cargo run -p tpt-window --example hello_window   # the smallest loop
cargo run -p tpt-window --example event_dump     # every event, printed
cargo run -p tpt-window --example input_state    # held keys and buttons, folded from events
cargo run -p tpt-window --example multi_window   # two windows, one queue
```

Then read the three things that differ from most window libraries:

1. **You own the loop.** Call `poll_events` or `step` yourself. `step(timeout)` waits its full
   timeout when nothing happens, so when a display paces your loop, pass `Duration::ZERO`.
2. **Events go through a queue you own.** `queue.dropped()` counts every event lost on the way.
   Check it while you develop.
3. **Modifiers are events.** `WindowEvent::ModifiersChanged` reports Shift, Control, Alt and
   Super. Keys are reported by physical position (`physical_key`) and by layout (`key_code`).

Pitfalls to know before you ship: on Windows the window is modal while the user drags or resizes
it, so your loop stops; on the web the page must drive `tick` from `requestAnimationFrame`.

## Which crate do I need

| You want | Use |
|----------|-----|
| A window and events on a desktop or the web | `tpt-window` (the facade) |
| Tests without a display | `tpt-window-test-utils` (headless backend) |
| Game controllers, kept out of the window stream | `tpt-window-gamepad` |
| Capability-gated, signed input for tpt-archon | `tpt-window-archon` |
| Passing frames between processes on Linux | `tpt-window-memfd` |
| wgpu surface and device setup | `tpt-window-wgpu` |
| Bevy ECS integration | `tpt-window-bevy` |
| An app written against winit 0.30 | `tpt-window-winit-compat` |
| Passing frames between processes on Windows | `tpt-window-shm` with `tpt-window-core::channel` |
| `ecs` resources (`WindowResource`, `InputEventQueue`) | `tpt-window-core` with its `ecs` feature |

## Platform support

| Target                         | Backend crate            | Status |
|--------------------------------|--------------------------|--------|
| Windows (x86_64, i686)         | `tpt-window-sys-win32`   | Implemented and tested, including touch, raw mouse and IME placement. Pen and transparency are not implemented. |
| macOS                          | `tpt-window-sys-cocoa`   | Implemented. Compile-checked only; not run on a Mac yet. |
| Linux, X11                     | `tpt-window-sys-x11`     | Implemented, including key repeat. Lifecycle test runs under Xvfb in CI. Compile-checked from Windows. |
| Linux, Wayland                 | `tpt-window-sys-wayland` | Implemented, including key repeat. Tested under headless Weston in CI, not on a real compositor. Compile-checked from Windows. |
| Web (`wasm32-unknown-unknown`) | `tpt-window-sys-wasm`    | Rendering checked in Edge with WebGPU. Input and resize checked in Chromium. Firefox and Safari not checked. |
| Anything else                  | none                     | `new_event_loop()` returns `WindowError::Unsupported`. |

On Linux, `new_event_loop()` uses Wayland when `WAYLAND_DISPLAY` is set and the
connection succeeds, and X11 otherwise.

### Feature status

Each cell says how the feature was checked. **Tested** means a test ran the real code on that
platform (the X11 and Wayland tests run in CI). **Compiled** means it type-checks and lints for the target, but has not run. **Partial**
means it works with a stated limit. **No** means it is not implemented, and the call is accepted
and has no effect.

| Feature                          | Windows | macOS    | X11     | Wayland  | Web     |
|----------------------------------|---------|----------|---------|----------|---------|
| Windows, events, close           | Tested  | Compiled | Tested  | Tested   | Tested  |
| Redraw requests                  | Compiled | Compiled | Compiled | No       | Compiled |
| Key repeat                       | Compiled | Compiled | Tested  | Compiled | Compiled |
| Layout key codes (`key_code`)     | Tested  | Compiled | Compiled | Compiled | Compiled |
| Modifier state                   | Tested  | Compiled | Compiled| Compiled | Compiled|
| IME commit                       | Tested  | Compiled | Partial (keysym, no IME) | Compiled | Compiled |
| IME composition and placement    | Partial (placement tested, real input method not) | No | No | No | No |
| Raw mouse motion                 | Tested  | No       | No      | No       | No      |
| Touch                            | Partial (down and move tested) | No | No | No | No |
| Monitors and current monitor     | Tested  | Compiled | Partial (one screen) | No | Partial (one screen) |
| Theme (light or dark)            | Tested  | No       | No      | No       | Compiled |
| Cursor visibility                | Tested  | Compiled | Compiled| No       | Compiled|
| Cursor grab                      | Tested (confine) | No | Compiled | No | Partial (lock only) |
| Window icon                      | Tested  | No       | Compiled| No       | No      |
| Size limits, resizable           | Partial (resizable tested; limits not) | Compiled | Compiled | Partial | No |
| Maximize and minimize            | Tested  | Compiled | Compiled| Partial  | No      |
| Decorations and fullscreen       | Tested  | Compiled | Compiled| Partial  | No      |
| Always on top                    | Tested  | Compiled | Compiled| No       | No      |
| File drop                        | Partial (drop tested by chunking only; no hover) | No | No | No | No |
| Cross-thread wake (`EventLoopProxy`) | Tested | No    | Compiled | Compiled | No   |
| Menu (Quit, Cmd+Q)               | No      | Compiled | No      | No       | No      |

## Crates in this workspace

| Crate                    | Purpose |
|--------------------------|---------|
| `tpt-window`             | Facade. Re-exports core and selects the backend for the target. Start here. |
| `tpt-window-core`        | Platform-agnostic events, window types, traits, the event queue, and the version 1 display protocol wire format. |
| `tpt-window-sys-win32`   | Windows backend. |
| `tpt-window-sys-cocoa`   | macOS backend. |
| `tpt-window-sys-x11`     | X11 backend. |
| `tpt-window-sys-wayland` | Wayland backend. |
| `tpt-window-sys-wasm`    | Web backend. |
| `tpt-window-test-utils`  | Headless backend for tests, CI and fuzzing. |
| `tpt-window-gamepad`     | Optional gamepad input over `gilrs`. Not part of the windowing layer. |
| `tpt-window-archon`      | Capability bridge to tpt-archon: scope mapping, signed input events and a gated router. Works without tpt-archon. |
| `tpt-window-shm`         | Shared-memory transport for the display protocol (Windows). |
| `tpt-window-winit-compat` | A winit 0.30 style `ApplicationHandler` and `run_app` over tpt-window's `step` loop, for migrating winit apps. |
| `tpt-window-memfd`       | Shared-memory transport for the display protocol (Linux). |
| `tpt-window-wgpu`        | wgpu surface and device helpers for a window. |
| `tpt-window-bevy`        | Bevy ECS integration: the window, input and event loop as resources. |
| `tpt-window-bench`       | Benchmark against winit 0.30 on the same loop. Not published. |

Each crate has its own README and CHANGELOG.

## Building and testing

```sh
cargo test --workspace
cargo deny check
```

Platform-specific tests need the matching desktop, so run the Win32 tests on Windows
and the X11 and Wayland tests on Linux. CI covers the rest. The web backend builds
with `cargo check -p tpt-window-sys-wasm --target wasm32-unknown-unknown`, and its
Chromium check is in `tpt-window-sys-wasm/browser-check/`.

## Documentation

- Migrating from winit 0.30: [MIGRATION.md](MIGRATION.md), covering the event and window mapping and the known gaps.
- Design specification: [spec.txt](spec.txt).
- Implementation status and open decisions: [todo.md](todo.md).
- API docs: `cargo doc --workspace --open`.

## Scope

tpt-window is not a GUI toolkit (no widgets, layout or theming), not a compositor, and
not a game framework. It is not a general-purpose winit replacement. It targets
engine and compositor workloads.

## Contributing

This project accepts issues only. Open one at
<https://github.com/tpt-solutions/tpt-window/issues>. See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.
