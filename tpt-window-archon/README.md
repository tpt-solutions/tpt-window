# tpt-window-archon

> **Status: scaffold.** This crate currently contains no code. It is a placeholder
> for the capability bridge between [tpt-window](../tpt-window) and
> [tpt-archon](https://github.com/tpt-solutions/tpt-archon). Do not depend on it
> yet; it exports nothing.

## Purpose

tpt-archon is a capability-based microkernel. Its capability types are
`no_std` and use `Rc`, so they are not `Send`. The bridge will map tpt-window's
access scopes onto archon capabilities, so that windows, input and displays are
granted and checked the same way as other kernel resources.

The design decisions recorded so far (see [todo.md](../todo.md), "Needs your
decision"):

- Window scopes map onto archon's `Resource::Channel` and `Resource::Device`
  capabilities, inside this crate.
- Event signing is done here, as an envelope around events. The core crate stays
  free of cryptography.
- archon is extended only if the mapping cannot express per-window scope.
- A standalone fallback is required, so tpt-window still works without archon.

The core crate already defines the matching types (`CapabilityToken`,
`WindowScope`, `InputScope`, `DisplayScope`, `SurfaceHandle`) in
`tpt-window-core::security`.

## Not yet done

- The bridge itself (todo.md, Phase 4).
- Reading the rest of the archon sources that the design depends on
  (`tpt-archon-kernel` `ipc.rs` and `driver.rs`, and `tpt-archon-bridge`
  `grant.rs`).
- Verifying that signed input events prevent spoofing.
- Capability-gated input routing tests.

## Usage

None yet. The crate depends on `tpt-window-core` but exports nothing; `src/lib.rs`
is a placeholder comment.

## Documentation

- Changes: [CHANGELOG.md](CHANGELOG.md)
- Plan and open decisions: [todo.md](../todo.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
