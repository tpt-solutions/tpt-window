# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- `MemfdMemory`: a Linux shared-memory region created with `memfd_create`. It implements
  `tpt_window_core::channel::RingMemory`, so a `FrameRing` runs over it.
- Sharing by file descriptor: a region is shared only with the process the descriptor is
  passed to (for example over a Unix socket with `SCM_RIGHTS`).

### Notes
- Compile-checked for Linux. The tests run in CI; they have not yet been run on a developer
  machine.
