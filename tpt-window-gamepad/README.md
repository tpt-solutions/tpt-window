# tpt-window-gamepad

Optional gamepad input for [tpt-window](../tpt-window), built on
[`gilrs`](https://crates.io/crates/gilrs) 0.11.

Gamepads are kept out of the windowing layer on purpose. This crate adds no
dependencies to `tpt-window` itself. Events are expressed in tpt-window's own
types, so your code never imports `gilrs`.

## Usage

```toml
[dependencies]
tpt-window-gamepad = { git = "https://github.com/tpt-solutions/tpt-window" }
```

```rust,no_run
use tpt_window_gamepad::{GamepadEvent, Gamepads};

let mut pads = Gamepads::new().expect("gamepad backend");
let mut events = Vec::new();

for _frame in 0..600 {
    pads.poll(&mut events); // call once per frame; never blocks
    for ev in events.drain(..) {
        if let GamepadEvent::Button { button, pressed: true, .. } = ev {
            println!("pressed {button:?}");
        }
    }
    // ...update and render here, then wait for the next frame.
}
```

`Gamepads::poll` appends the changes since the last call to a `Vec` that you own.
Reuse that `Vec` across frames so it is not reallocated every frame.

## Events

`GamepadEvent` (marked `#[non_exhaustive]`):

| Variant                              | Meaning                                                       |
|--------------------------------------|---------------------------------------------------------------|
| `Connected(GamepadId)`               | A pad was plugged in.                                         |
| `Disconnected(GamepadId)`            | A pad was unplugged. Its id is not reused while the program runs. |
| `Button { id, button, pressed }`     | A button went down (`pressed: true`) or up.                   |
| `Axis { id, axis, value }`           | An axis moved to `value` (`f32`).                             |

`GamepadButton` and `GamepadAxis` are this crate's own enums. `GamepadId` is
re-exported from `tpt-window-core`.

## Platform support

The backend is whatever `gilrs` supports:

- Windows: compiles and clippy-clean. Unit tests pass.
- Linux: needs the `libudev` development package at build time (`libudev-dev` on
  Debian and Ubuntu). CI installs it.
- macOS: compiles and clippy-clean.

No real controller has been tested yet, so treat device behaviour as unverified.

## Testing

```sh
cargo test -p tpt-window-gamepad
```

The unit tests run without a physical controller.

## Changes

See [CHANGELOG.md](CHANGELOG.md).

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
