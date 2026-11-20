# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- `create(&window)`: returns a `Gpu` (device and queue) and a `GpuSurface` for a
  tpt-window window.
- `GpuSurface::resize`, for reconfiguring the surface on `Resized`.
- `GpuSurface::acquire` and `Frame::present`. `acquire` returns `None` when no frame is
  available, so the loop can skip it.
- `GpuError` for surface and adapter failures.
- `wgpu_triangle` example. It ran for three seconds on an NVIDIA GPU on Windows without
  errors; the picture was not checked by eye.

### Notes
- The default `Fifo` present mode makes `present` wait for the display, which paces the
  loop. The loop must not also wait in `step`.
