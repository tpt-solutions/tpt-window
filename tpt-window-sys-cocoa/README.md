# tpt-window-sys-cocoa

The macOS backend for [tpt-window](../tpt-window). It drives AppKit through
[`objc2`](https://crates.io/crates/objc2) and [`objc2-app-kit`](https://crates.io/crates/objc2-app-kit).

Most applications should use the facade, which selects this backend on macOS:

```toml
[dependencies]
tpt-window = { git = "https://github.com/tpt-solutions/tpt-window" }
```

This crate compiles only for macOS. On other platforms it is empty.

## How it works

- **Main thread.** AppKit runs on the thread that created the `EventLoop`, which
  is enforced with `MainThreadMarker`. Other threads can only queue commands,
  which are applied on the next poll.
- **Input.** The backend pumps `NSEvent`s, translates them into tpt-window events,
  and then forwards each event to `NSApplication::sendEvent`. AppKit keeps its
  normal behaviour, and no custom `NSView` subclass is needed.
- **Close.** There is no window delegate yet, so clicking the close button closes
  the window without asking. The backend reports `CloseRequested` followed by
  `Destroyed`.

## Known gaps

- **Close cannot be vetoed.** Refusing a close needs a window delegate, which is
  not written yet.
- **No cursor enter/leave events.** These need an `NSTrackingArea`, which is not set
  up.
- **No IME.** Composition (`ImeEvent::Preedit`) is not started for Cocoa.
- **No DPI verification on Retina displays** beyond the scale factor reported by
  AppKit. See the cross-platform DPI item in [todo.md](../todo.md).

## Testing

```sh
cargo test -p tpt-window-sys-cocoa
```

`tests/real_window.rs` uses `harness = false` in `Cargo.toml`, because AppKit must
be driven from the process main thread, which libtest does not provide. It has its
own `main`.

The test runs on the macOS CI runner. It has been compiled and clippy-checked on
macOS, but it has not yet been seen passing on CI.

## Documentation

- Changes: [CHANGELOG.md](CHANGELOG.md)
- Open items: [todo.md](../todo.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
