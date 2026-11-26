# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- Web backend on `web-sys` and `wasm-bindgen`, with one `<canvas>` per window.
- DOM listeners for keyboard, mouse, wheel, touch, focus and `ResizeObserver`,
  feeding a pre-allocated ring that `poll` drains.
- `requestAnimationFrame`-driven redraw.
- Browser check harness in `browser-check/`: runs the backend in Chromium through
  Playwright and checks pointer, wheel, keyboard, committed text and canvas resize.
  It is not part of the workspace. CI runs it in a `browser` job, in Chromium, after building the wasm harness.
- Window state, cursor, icon, monitor, theme and proxy support from the expanded core API.
  Compile-checked; the browser check covers input and resize only.

### Notes
- `wait` behaves like `poll`, because a browser cannot block. Callers drive the
  loop from `requestAnimationFrame`.
- Rendering verified in Edge with WebGPU through tpt-nova. Input and resize are
  checked in Chromium by the browser check harness. Firefox and Safari are not
  checked.
- No IME composition.
