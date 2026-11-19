# tpt-window-sys-win32

The Windows backend for [tpt-window](../tpt-window). It is built on
[`windows-sys`](https://crates.io/crates/windows-sys) (the raw FFI crate from the
windows-rs project).

Most applications should use the facade, which selects this backend on Windows:

```toml
[dependencies]
tpt-window = { git = "https://github.com/tpt-solutions/tpt-window" }
```

Depend on this crate directly only if you need Win32-specific items and do not
want the facade.

## How it works

- **Message pump.** The window procedure runs inside `poll` and `step`, which
  dispatch messages themselves. Each message is translated into an `Event` and
  pushed into a thread-local ring buffer. `poll` then moves those events into the
  caller's sink. This path does not allocate.
- **Cross-thread setters.** `Window` setters can be called from any thread. Each
  one queues a command and posts a message. The loop thread applies the command on
  its next poll.
- **DPI.** Per-monitor-v2 DPI awareness is enabled when the loop is created.

## What is implemented

- Window creation, title, size and min/max size, position, focus, visibility.
- Keyboard (physical key codes), mouse buttons, wheel, text from `WM_CHAR`.
- IME composition as `ImeEvent::Preedit` from `WM_IME_COMPOSITION`. Committed
  text arrives through the `WM_CHAR` path as `ImeEvent::Commit`.
- IME placement: `Window::set_ime_cursor_area` sets the composition and candidate
  windows, so the candidate list opens under the text being composed.
- Touch down and move through `WM_TOUCH`, with the `WindowEvent::Touch` event.
- Raw mouse motion as `WindowEvent::RawMouseMotion` from `WM_INPUT`. Only one
  window per process receives raw motion: the last one created.
- Resize and move events.

## What is not implemented

- **Touch lift** (`TouchPhase::Ended`) is not verified. Injected lifts time out
  on the development machine.
- **Pen input.** Windows has no pen injection API, so the planned design is
  `CursorMoved` with pressure from `WM_POINTER`. It has not been built.
- **`transparent` windows.** Extending the DWM frame into the client area gives a
  white window, not a see-through one. Real transparency needs a DXGI composition
  swapchain with composite alpha, which belongs in the renderer.
- **Verification of IME** with a real Japanese or Chinese input method is
  outstanding. The development machine and the CI runners have none installed, so
  the candidate-window placement is checked by reading the IME context back, not by
  typing.

## Platform support

- Builds and tests for `x86_64-pc-windows-msvc` and `i686-pc-windows-msvc`.
  The 32-bit test suite is a CI job.
- Requires Windows 10 or later for per-monitor-v2 DPI awareness.

## Testing

```sh
cargo test -p tpt-window-sys-win32
```

The tests create real Win32 windows on the current desktop:

- `tests/real_window.rs`: window lifecycle and event delivery against a hidden
  window, including a check that 2000 posted messages cause no allocations.
- `tests/raw_input.rs`: raw mouse input, verified with `SendInput`.
- `tests/touch.rs`: touch down and move, verified with injected touch against a
  visible window.
- `tests/ime_area.rs`: the IME composition and candidate areas set by
  `set_ime_cursor_area`, read back through the window's IME context.

## Documentation

- Changes: [CHANGELOG.md](CHANGELOG.md)
- Open items and decisions: [todo.md](../todo.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
