# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- X11 backend on Xlib, loaded at runtime through `x11-dl`. Xlib rather than `x11rb`
  because wgpu's Xlib surface needs the `Display*`.
- Window creation, close through `WM_DELETE_WINDOW`, key and mouse input, wheel,
  and configure-driven resize and move.
- Scale factor from the `Xft.dpi` resource, read when the loop opens.
- Lifecycle test (`tests/real_window.rs`) in a CI job under Xvfb.

### Known gaps
- No key repeat.
- No IME.
- Scale factor is read once and is not updated if `Xft.dpi` changes.
