# tpt-window-sys-wayland

The Wayland backend for [tpt-window](../tpt-window) on Linux. It is built on
[`smithay-client-toolkit`](https://crates.io/crates/smithay-client-toolkit) (sctk) 0.19.

Most applications should use the facade. On Linux it uses Wayland when
`WAYLAND_DISPLAY` is set and the connection succeeds, and otherwise uses X11:

```toml
[dependencies]
tpt-window = { git = "https://github.com/tpt-solutions/tpt-window" }
```

## How it works

- **xdg-shell.** Windows are xdg toplevels using the stable `xdg_wm_base`
  protocol. Version negotiation (bind the lower of the compositor's version and the
  latest supported) is a design target; see [todo.md](../todo.md).
- **Configure drives size.** Wayland does not let a client choose its own size or
  position. The compositor's configure events produce `Resized`.
- **Close.** Close requests are reported as `CloseRequested`. The application
  decides whether to drop its window, so it can veto a close.
- **Keyboard.** The xkb keymap is handled through sctk. Text input comes from
  `ImeEvent::Commit`.
- **Key repeat.** sctk's repeat timer runs on the event loop. A held key sends
  repeated `Pressed` events through the same path as a normal key press.
- **Runtime loading.** `wayland-backend` is built with `dlopen`, so compiling does
  not need the Wayland development packages.

## Limitations by design

Some tpt-window commands have no Wayland equivalent. They are ignored:

- `SetSize`: clients cannot choose their size.
- `SetPosition`: clients cannot place their own windows.
- `SetVisible`: there is no protocol to hide a toplevel.

Code that depends on these should check the resulting `Resized` and `Moved`
events rather than assume the request took effect.

## Known gaps

- **No IME composition.** Text-input v3 is not implemented, so `Preedit` is never
  produced.

## Platform support

- Builds on Linux only.
- The integration test has not been run on a real compositor. CI runs it with
  headless Weston.

## Testing

```sh
cargo test -p tpt-window-sys-wayland
```

`tests/real_window.rs` runs the window lifecycle. CI runs it in a job with headless
Weston. That job has not yet been seen passing.

## Documentation

- Changes: [CHANGELOG.md](CHANGELOG.md)
- Open items: [todo.md](../todo.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
