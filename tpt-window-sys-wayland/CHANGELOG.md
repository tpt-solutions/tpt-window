# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- Wayland backend on `smithay-client-toolkit` 0.19: xdg toplevels, seat keyboard
  and pointer, configure-driven resize, and close requests.
- Client library loaded at runtime (`dlopen`), so building does not need Wayland
  development packages.
- Lifecycle test (`tests/real_window.rs`) in a CI job with headless Weston.
- Key repeat from sctk's repeat timer: a held key sends repeated `Pressed` events
  (text included), sharing the press path with the normal key event.

### Notes
- `SetSize`, `SetPosition` and `SetVisible` are ignored, because Wayland does not
  let clients set them.
- No IME composition yet.
- Not yet tested on a real compositor.
