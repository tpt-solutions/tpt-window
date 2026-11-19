# tpt-window-test-utils

A headless [tpt-window](../tpt-window) backend for tests, CI and fuzzing. It opens
no OS window. Instead, the test script injects events, and the code under test
reads them back through the same `EventLoop` and `EventQueue` that a real backend
uses.

Use it to test code that consumes tpt-window events, without a display server.

```toml
[dev-dependencies]
tpt-window-test-utils = { git = "https://github.com/tpt-solutions/tpt-window" }
```

## Usage

```rust
use tpt_window_core::*;
use tpt_window_test_utils::headless;

let (mut event_loop, handle) = headless();
let window = WindowBuilder::new().build(&event_loop).unwrap();
handle.inject(window.id(), WindowEvent::CloseRequested);

let mut queue = EventQueue::with_capacity(16);
event_loop.poll_events(&mut EventSink::new(&mut queue));
assert_eq!(queue.pop().unwrap().event, WindowEvent::CloseRequested);
```

`headless()` returns an `EventLoop` and a `HeadlessHandle`. The handle is the
script side:

| Method                    | Effect                                                         |
|---------------------------|----------------------------------------------------------------|
| `inject(window, event)`   | Queues an event for the next `poll_events` or `step`.           |
| `reserve(additional)`     | Pre-allocates space in the injection queue before a measured run. |

Because the handle and the loop are separate values, a test can inject events
from one place and drive the loop from another.

## What it is for

- **Unit and integration tests** of code that handles `WindowEvent`s, with no
  display needed. This works on CI runners that have no GPU or desktop.
- **Yield-point contracts.** `tests/yield_sync.rs` checks the frame-boundary
  behaviour of `YieldPoint`: one yield per poll, the last one in each batch,
  numbered in order, and lost yields counted in `dropped()`.
- **Fuzzing** the event path with arbitrary injected sequences.

## Limits

- It does not create a surface, so there is nothing to hand to wgpu.
- It does not model OS timing or real input devices. Input-device behaviour must
  be tested against the real backend (see the `tests/real_window.rs` files in each
  `tpt-window-sys-*` crate).

## Testing

```sh
cargo test -p tpt-window-test-utils
```

## Documentation

- Changes: [CHANGELOG.md](CHANGELOG.md)
- Design: [spec.txt](../spec.txt)

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](../LICENSE-APACHE))
- MIT license ([LICENSE-MIT](../LICENSE-MIT))

at your option.
