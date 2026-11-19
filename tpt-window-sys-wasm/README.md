# tpt-window-sys-wasm

The web backend for [tpt-window](../tpt-window). It targets `wasm32-unknown-unknown`
and uses [`web-sys`](https://crates.io/crates/web-sys) and `wasm-bindgen`. Each
window is a `<canvas>` element.

Most applications should use the facade, which selects this backend on `wasm32`:

```toml
[dependencies]
tpt-window = { git = "https://github.com/tpt-solutions/tpt-window" }
```

## How it works

- **Canvas per window.** Each window owns one canvas appended to `<body>`. The
  backing store is sized in physical pixels: CSS size multiplied by
  `devicePixelRatio`.
- **DOM listeners feed a ring.** Keyboard, mouse, wheel, touch, focus and
  `ResizeObserver` callbacks push events into a pre-allocated ring. `poll` drains
  the ring into the caller's sink.
- **Listeners are kept for the page lifetime.** The closures are leaked with
  `forget`, which is intentional.
- **Threads.** `WindowBackend` requires `Send + Sync`. The backend is sound
  because `wasm32-unknown-unknown` is single-threaded without the `atomics`
  target feature.

## Driving the loop in a browser

A browser cannot block, so `wait` behaves like `poll`. Drive the loop from
`requestAnimationFrame`, calling `poll` once per frame, rather than in a
blocking `loop`. The core crate uses `web-time` on this target, because
`std::time::Instant` panics in the browser.

## Status

- **Rendering verified** in Microsoft Edge with WebGPU, using tpt-nova (headless
  Edge via Playwright, on an RTX 3050). A screenshot was checked.
- **Input and resize** have not yet been checked in a browser.
- **No IME composition.** Only committed text is delivered.
- Compile- and clippy-checked for `wasm32-unknown-unknown` in CI.

## Testing

```sh
cargo check -p tpt-window-sys-wasm --target wasm32-unknown-unknown
```

There are no in-browser tests in this repository yet. Browser verification has so
far been done through tpt-nova.

## Documentation

- Changes: [CHANGELOG.md](CHANGELOG.md)
- Open items: [todo.md](../todo.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
