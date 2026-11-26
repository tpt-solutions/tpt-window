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
- Re-exports the core additions made since the first changelog entry:
  `Window::set_ime_cursor_area` and the `protocol` module (version 1 wire format).
- Re-exports the core types for icons, monitors, themes, proxies, path assembly and cursor grab.
- Optional `ecs`, `archon` and `gamepad` features. `archon` and `gamepad` are re-exported as
  `tpt_window::archon` and `tpt_window::gamepad`.
- Examples: `multi_window`, `hello_run`, `input_state`, `ecs_resource`, `file_drop`, `gamepad`,
  `ime_text_field` and `record_replay`.
- Surface target test (`tests/surface_target.rs`).

### Changed
- The event codec (in `tpt-window-core`) rejects a `ModifiersChanged` byte with bits outside
  the four modifier flags. Such bytes were accepted before and are now an error.
