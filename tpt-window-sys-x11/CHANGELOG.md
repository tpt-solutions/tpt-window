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
- Key repeat: detectable auto-repeat is enabled when the loop opens, so a held key
  sends repeated `Pressed` events (text included) with no release in between. Servers
  without detectable auto-repeat keep the release and press pairs.
- Scale factor from the `Xft.dpi` resource, read when the loop opens.
- Lifecycle test (`tests/real_window.rs`) in a CI job under Xvfb.
- Key repeat test (`tests/key_repeat.rs`): drives a held key through XTest and checks
  the press counts.

### Known gaps
- No IME.
- Scale factor is read once and is not updated if `Xft.dpi` changes.
