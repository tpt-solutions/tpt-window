# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- macOS backend on AppKit through `objc2` and `objc2-app-kit`: window creation,
  queued commands applied on poll, and `NSEvent` input forwarded to
  `NSApplication::sendEvent`.
- A window delegate turns the close button into `CloseRequested` and keeps the window
  open, so the application closes it by dropping its window, as on Win32 and Wayland.
- Cursor enter and leave: a tracking area on the content view reports `CursorEntered`
  and `CursorLeft`.
- `tests/real_window.rs` with a custom `main` (`harness = false`), run on the macOS
  CI runner.
- Window setters are applied on poll, as on the other backends. Compile-checked only.

### Known gaps
- No IME composition.
- Compiled for macOS only so far; the changes above have not been run on a Mac.
