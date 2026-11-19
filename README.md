# tpt-window

Engine-first windowing, input and surface management for the TPT AV Stack
(tpt-nova, tpt-appfront, tpt-shell, tpt-archon).

- The caller owns the main loop (no closure hijacking).
- Zero heap allocation on the event hot path.
- Window state exposed as ECS-friendly data.
- Explicit AI-agent yield points per frame.
- Permissive-only dependency tree, enforced by `cargo-deny`.

**Status:** pre-release (0.1.0), not yet on crates.io. Per-platform verification is
tracked in [todo.md](todo.md). See also [spec.txt](spec.txt).

Coming from winit 0.30? See [MIGRATION.md](MIGRATION.md) for the event and window mapping,
and the gaps.

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE))
- MIT license ([LICENSE-MIT](LICENSE-MIT))

at your option.

This project accepts issues only; see [CONTRIBUTING.md](CONTRIBUTING.md).
