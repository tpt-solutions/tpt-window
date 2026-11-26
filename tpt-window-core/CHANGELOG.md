# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- Platform-agnostic `EventLoop`, `Window` and `WindowBuilder`.
- `EventLoopBackend` and `WindowBackend` traits for backend implementors.
- `EventQueue` and `EventSink`: caller-owned, pre-allocated event storage. A full
  queue returns `EventStatus::BufferFull`.
- `RingBuffer` and `OverflowPolicy`.
- `WindowEvent`, `Event` and input types (`KeyCode`, `PhysicalKey`,
  `ElementState`, `MouseButton`, `MouseScrollDelta`, `TouchEvent`, `TouchPhase`).
  All events are `Copy`.
- `ImeEvent` with `Preedit` and `Commit`, and fixed-size inline text buffers.
- Physical and logical DPI types.
- `YieldPoint`, emitted once per step for AI-agent yield points. Lost yields are
  counted in `dropped()`.
- `WindowEvent::Touch` and `WindowEvent::RawMouseMotion`.
- Security types for capability-gated access: `CapabilityToken`, `WindowScope`,
  `InputScope`, `DisplayScope` and `SurfaceHandle`.
- `ecs` feature: `WindowResource` and `InputEventQueue` for ECS engines.
- Property tests for the event queue and an allocation-counting test of the event
  hot path.
- `protocol` module: version 1 of the display-server wire format (`Message`, `encode`,
  and a decoder that checks the length against `MAX_FRAME_LEN` before it allocates),
  with `negotiate` for the version handshake. It has no transport.
- `Window::set_ime_cursor_area` and `WindowCommand::SetImeCursorArea`, which place the
  IME composition and candidate windows. Applied on Win32; other backends ignore it.
- `codec` module: binary wire codec for events, with a round-trip test
  (`tests/codec_roundtrip.rs`).
- `channel` module: the `RingMemory` trait for shared-memory regions, and `FrameRing` over it
  for protocol frames.
- `keymap` module: layout key-code tables used by the backends, tested in the module.
- `Theme`, `MonitorInfo` and `Icon` (with `IconError`) types.
- `proxy` module: `EventLoopProxy`, for waking the loop from another thread.
- `record` and `tap` modules: record events to a log and replay them, and observe events as
  they pass through the sink.
- `WindowEvent::ModifiersChanged` and input state tracking in `input`.
- `PathAssembler` for file-drop paths, and `ImeText` alongside `InlineText`.
- `WindowCommand` variants for minimum and maximum size, resizable, maximized and minimized.
- Fuzz targets for the codec, the protocol decoder, the frame ring and the session-log verifier,
  in the workspace's `fuzz/` directory. CI runs each for a minute on nightly.

### Changed
- Uses `web-time` on `wasm32`, because `std::time::Instant` panics in browsers.
- Yield-point contract is tested: one yield per poll, numbered in order.
- `PROTOCOL_VERSION` is 1. It was 0 while the protocol was a skeleton.
- Public `KeyCode` variants and the DPI pair fields are documented. The crate passes
  `missing_docs` without suppression.
- The codec rejects a `ModifiersChanged` byte with bits outside the four modifier flags
  (`CodecError::BadValue`). Such a byte used to decode by masking, so re-encoding gave
  different bytes. The public `ModifiersState::from_bits` still masks. Found by the
  `codec_decode` fuzz target.
