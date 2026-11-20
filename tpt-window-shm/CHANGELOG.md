# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- `SharedMemory`: a named Windows file mapping that implements
  `tpt_window_core::channel::RingMemory`, with `create` and `open`.
- Tests: a producer and consumer on two views of one mapping, 50 000 frames in order;
  duplicate names refused; a too-small mapping refused.
