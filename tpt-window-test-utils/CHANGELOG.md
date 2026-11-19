# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- Headless backend: `headless()` returns an `EventLoop` and a `HeadlessHandle`.
- `HeadlessHandle::inject` for scripted events, and `reserve` for pre-allocation.
- Tests covering injected events.
- `tests/yield_sync.rs`: the `YieldPoint` frame-boundary contract, including
  counting of lost yields.
