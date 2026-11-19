# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- Win32 backend on `windows-sys`: window procedure, message pump, keyboard, mouse,
  wheel, text from `WM_CHAR`, focus, resize and move, min/max size.
- Per-monitor-v2 DPI awareness, enabled when the loop is created.
- Setters callable from any thread: commands are queued and applied on the loop
  thread.
- Touch down and move from `WM_TOUCH` (`WindowEvent::Touch`).
- Raw mouse motion from `WM_INPUT` (`WindowEvent::RawMouseMotion`). One window per
  process receives it.
- IME composition as `ImeEvent::Preedit`, from `WM_IME_COMPOSITION`.
- Build and test as `i686-pc-windows-msvc`, with a CI job.
- Tests: `real_window.rs`, `raw_input.rs` and `touch.rs`.

### Known gaps
- Touch lift (`TouchPhase::Ended`) is not verified.
- Pen input is not implemented.
- `transparent` windows are not implemented; they need a renderer-side DXGI
  composition swapchain.
- IME candidate window placement is not implemented, and IME has not been verified
  with a real input method.
