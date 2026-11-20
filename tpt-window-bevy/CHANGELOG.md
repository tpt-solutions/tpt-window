# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- `install` and `install_with`: put the window, its input queue and the event loop into a
  Bevy `World` as resources (`TptWindow`, `TptInput` and `TptLoop`).
- `pump_system`: runs the event loop once per schedule run and fills the input queue.
- Built on `bevy_ecs` 0.16.
- Headless test: a scripted event reaches the input resource, with no display needed.

### Notes
- The event loop is a non-send resource, so the schedule must run on the thread that
  created the loop.
