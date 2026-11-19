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

Run the examples with:

```sh
cargo run -p tpt-window --example hello_window
cargo run -p tpt-window --example event_dump
```

Minimum supported Rust version: **1.85** (edition 2024).

## Platform support

| Target                         | Backend crate            | Status |
|--------------------------------|--------------------------|--------|
| Windows (x86_64, i686)         | `tpt-window-sys-win32`   | Implemented and tested, including touch, raw mouse and IME placement. Pen and transparency are not implemented. |
| macOS                          | `tpt-window-sys-cocoa`   | Implemented. Compile-checked only; the CI test has not yet been seen passing. |
| Linux, X11                     | `tpt-window-sys-x11`     | Implemented, including key repeat. Lifecycle test runs under Xvfb in CI. |
| Linux, Wayland                 | `tpt-window-sys-wayland` | Implemented, including key repeat. Tested under headless Weston in CI, not on a real compositor. |
| Web (`wasm32-unknown-unknown`) | `tpt-window-sys-wasm`    | Rendering checked in Edge with WebGPU. Input and resize checked in Chromium. Firefox and Safari not checked. |
| Anything else                  | none                     | `new_event_loop()` returns `WindowError::Unsupported`. |

On Linux, `new_event_loop()` uses Wayland when `WAYLAND_DISPLAY` is set and the
connection succeeds, and X11 otherwise.

IME composition (`ImeEvent::Preedit`) is implemented on Win32 only. Other backends
deliver committed text through `ImeEvent::Commit`.

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
| `tpt-window-shm`         | Shared-memory transport for the display protocol (Windows). Not yet in the workspace. |
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
