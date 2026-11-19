# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- `Gamepads` with a non-blocking `poll(&mut Vec<GamepadEvent>)` over `gilrs` 0.11.
- `GamepadEvent` (`Connected`, `Disconnected`, `Button`, `Axis`), `GamepadButton`,
  `GamepadAxis` and `GamepadError`, all using tpt-window's own types.
- Linux builds need `libudev` at build time; CI installs it.
