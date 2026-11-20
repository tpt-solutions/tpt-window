# tpt-window-sys-x11

The X11 backend for [tpt-window](../tpt-window) on Linux. It uses Xlib, loaded
at runtime through [`x11-dl`](https://crates.io/crates/x11-dl).

Most applications should use the facade. On Linux it prefers Wayland when
`WAYLAND_DISPLAY` is set and falls back to X11 otherwise:

```toml
[dependencies]
tpt-window = { git = "https://github.com/tpt-solutions/tpt-window" }
```

## Why Xlib and not xcb

wgpu's Xlib surface needs the `Display*` pointer, and the pure-Rust `x11rb`
client does not expose it. Xlib is therefore the backend's native API.

## How it works

- **Thread affinity.** Every Xlib call happens on the thread that created the
  `EventLoop`. Handles used from other threads queue commands, which are applied on
  the next poll.
- **Close.** Closing goes through `WM_DELETE_WINDOW`, so the application can veto
  it. A close the application accepts destroys the window.
- **Key repeat.** Detectable auto-repeat (`XkbSetDetectableAutoRepeat`) is turned on
  when the loop opens, so a held key sends one press per repeat and no release in
  between. If the server cannot do it, a held key arrives as release and press pairs
  instead.
- **Errors.** X11 errors are handled by a no-op error handler instead of exiting
  the process.
- **Scale factor.** Read once from the `Xft.dpi` resource when the loop opens
  (96 dpi = 1.0). It falls back to 1.0 if the resource is missing. A later change
  to `Xft.dpi` is not picked up.

## What is implemented

- Window creation, title, size, min/max size and position.
- Key input with text, mouse buttons, and the wheel.
- Key repeat: a held key sends repeated `Pressed` events, through detectable
  auto-repeat (see below).
- Configure-driven resize and move.
- `WM_DELETE_WINDOW` close handling.
- Window setters for the core state commands, applied on the loop thread. These are
  compile-checked from Windows and not yet run under X11.

## Known gaps

- **No IME.** XIM composition is not started.
- **Scale is read once** (see above).

## Platform support

- Builds on Linux only. Only Linux builds include this crate's code.
- Requires an X server at runtime.

## Testing

```sh
cargo test -p tpt-window-sys-x11
```

- `tests/real_window.rs` runs the window lifecycle. CI runs it under Xvfb in a
  dedicated job. That job has not yet been seen passing.
- `tests/key_repeat.rs` drives a held key through XTest and checks the press counts.

## Documentation

- Changes: [CHANGELOG.md](CHANGELOG.md)
- Open items: [todo.md](../todo.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
