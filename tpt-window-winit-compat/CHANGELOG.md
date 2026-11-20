# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- `ApplicationHandler` trait and `run_app`, with the callback shape of winit 0.30:
  `resumed`, `window_event`, `about_to_wait` and `exiting`.
- `ActiveLoop`, with `create_window`, `exit`, `proxy`, `set_control_flow` and
  `control_flow`.
- `ControlFlow` (`Wait`, `Poll` and `WaitUntil`), as in winit.
- `IDLE_WAIT` (16 ms): the longest wait while idle, so `about_to_wait` keeps running.
- Test `tests/run_app.rs`. It needs a display and runs on Windows only for now.
