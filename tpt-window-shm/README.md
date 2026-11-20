# tpt-window-shm

Shared-memory transport for the tpt-window display protocol. Windows only.

`SharedMemory` is a named file mapping. It implements `RingMemory` from
`tpt-window-core::channel`, so a `FrameRing` of protocol frames can run over it. One side
creates the mapping and the other opens it by name. The unsafe code for the mapping, the
atomic indices and the release/acquire ordering is confined to this crate. Core stays
`#![forbid(unsafe_code)]`.

Status: the transport is tested with a producer and consumer on two separate views of the
same mapping, and across two processes (the test binary as the server). No
display-server peer exists yet.

- Changes: [CHANGELOG.md](CHANGELOG.md)
- Plan and open decisions: [todo.md](../todo.md)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
