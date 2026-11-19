# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- Facade crate re-exporting `tpt-window-core` and selecting the platform backend.
- `new_event_loop()`: Win32 on Windows, Cocoa on macOS, Wayland (with X11 fallback)
  on Linux, and the web backend on `wasm32`. Returns `WindowError::Unsupported`
  elsewhere.
- `hello_window` and `event_dump` examples.
