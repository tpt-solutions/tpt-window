# tpt-window-memfd

Shared memory for the tpt-window display protocol on Linux, built on `memfd_create`.

`MemfdMemory` implements `tpt_window_core::channel::RingMemory`, so a `FrameRing` runs over it.
Two processes share the region by passing its file descriptor, for example over a Unix socket with
`SCM_RIGHTS`, and each maps the same bytes. This is the Linux counterpart of `tpt-window-shm`, which
uses a named file mapping on Windows.

## Status

- Compile-checked for Linux. The tests run in CI, which has a real kernel. They have not been run
  on a developer machine yet.
- The region is shared only as far as the descriptor is passed. Access control is the descriptor's
  (a region can be sent only to a process you hand the descriptor to).

## Testing

```sh
cargo test -p tpt-window-memfd
```

## Changes

See [CHANGELOG.md](CHANGELOG.md).

## License

Licensed under either of Apache License, Version 2.0 or MIT license, at your option.
