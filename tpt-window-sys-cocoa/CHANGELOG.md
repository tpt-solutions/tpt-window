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
- `tests/real_window.rs` with a custom `main` (`harness = false`), run on the macOS
  CI runner.

### Known gaps
- Close cannot be vetoed (no window delegate).
- No cursor enter/leave events (no tracking area).
- No IME composition.
