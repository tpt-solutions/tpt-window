# tpt-window Todo

Tracks the phased rollout from [spec.txt](spec.txt). License: MIT OR Apache-2.0, TPT Solutions.

## Decisions (locked)
- **Ring buffer:** caller-owned, drop-newest. A full buffer returns `EventStatus::BufferFull` and bumps a drop counter. Drop-oldest is a consumer-side policy. `EventLoop` does not hold `Arc<RingBuffer>`. (Replaces the spec's "SPSC + drop oldest".)
- **`WindowEvent` is strictly `Copy`.** Text/IME use fixed inline buffers (e.g. 32-byte UTF-8, chunked when longer).
- **Multi-window** per `EventLoop`. **IME** deferred to Phase 3, API-ready now. **Clipboard** deferred to tpt-archon.
- **Spec correction:** "xdg-shell v5" is wrong. v5/v6 were unstable revisions, obsolete since wayland-protocols 1.12. Target stable `xdg_wm_base`, bind min(compositor, latest). Verify the version in Phase 3.
- **Sibling crates:** only tpt-archon link known (https://github.com/tpt-solutions/tpt-archon). Use local stub types until real APIs are read.

## Phase 0: Repo & Governance
- [x] git init, .gitignore
- [x] LICENSE-MIT / LICENSE-APACHE (TPT Solutions, 2026), NOTICE
- [x] Workspace Cargo.toml, 9 crates compile (facade, core, 5 backends, archon, test-utils)
- [x] deny.toml
- [x] CI workflow (fmt, clippy, test on win/linux/mac, wasm32 check, cargo-deny)
- [x] README, CONTRIBUTING
- [ ] Verify `cargo deny check` passes locally (install cargo-deny)
- [ ] Create GitHub repo `tpt-solutions/tpt-window`, push, confirm CI green
- [ ] Pin toolchain (rust-toolchain.toml) and confirm MSRV (set to 1.85, unverified)
- [ ] CODE_OF_CONDUCT.md, SECURITY.md
- [ ] Update spec.txt with the Decisions above
- [ ] First commit

## Phase 1: Core & Proof of Concept (Weeks 1–2)
### Core types (`tpt-window-core`)
- [ ] `WindowError`, `WindowId`
- [ ] `PhysicalSize/Position`, `LogicalSize/Position`
- [ ] `KeyCode`, `PhysicalKey`, `ElementState`, `MouseButton`, `MouseScrollDelta`, `TouchPhase`, `TouchEvent`, `GamepadId`
- [ ] `ImeEvent` with inline fixed-size text
- [ ] `WindowEvent` (strictly `Copy`) + compile-time `Copy` and size assertion
- [ ] `raw-window-handle` 0.6 re-exports
### Ring buffer / sink
- [ ] `RingBuffer<T: Copy>`: fixed capacity, allocated once (default 1024)
- [ ] `EventSink`: `push`, `captured_count`
- [ ] `EventStatus` (`Accepted`, `BufferFull`, `Captured`)
- [ ] Drop counter + configurable policy
- [ ] Unit and property tests
### Traits and API
- [ ] `EventLoopBackend`, `WindowBackend` traits
- [ ] `EventLoop`: `new`, `poll_events`, `step`, `request_exit` (multi-window)
- [ ] `Window`: Arc handle, cached-snapshot getters (no syscalls), command-queue setters
- [ ] `WindowBuilder`: all fields, `with_capability` stubbed
- [ ] Stub `CapabilityToken`, `SurfaceHandle`, `WindowScope`/`InputScope`/`DisplayScope`
- [ ] `protocol.rs` skeleton (versioned)
- [ ] `YieldPoint` emitted once per frame after OS events drained
### Backends
- [ ] `tpt-window-test-utils`: headless backend + scripted event injection
- [ ] `tpt-window-sys-wasm`: canvas, keyboard/pointer/touch, resize, rAF
- [ ] `tpt-window-sys-win32`: wndproc, DPI awareness, raw input, vsync
- [ ] Facade `tpt-window`: cfg(target_os) re-exports
### Verification
- [ ] `tpt-window-core/tests/rt_safety.rs`: allocation-counting allocator, 0 allocs on poll + push
- [ ] Examples: `hello_window`, `event_dump`, caller-owned main loop
- [ ] **Exit:** Windows + WASM open a window and deliver events with zero hot-path allocation; CI green

## Phase 2: Engine Integration (Weeks 3–4)
- [ ] Get tpt-visual repo link; read tpt-nova, tpt-visual, tpt-appfront public APIs
- [ ] `WindowResource` and `InputEventQueue` ECS types (feature-gated)
- [ ] Replace winit in tpt-nova (Windows + WASM)
- [ ] AI-agent yield point in main loop + test that display refresh stays in sync
- [ ] tpt-visual: wgpu surface from raw handles; Resized reconfigures, RedrawRequested renders
- [ ] **Exit:** tpt-nova demo runs on tpt-window

## Phase 3: Hard Platforms (Weeks 5–8)
- [ ] `tpt-window-sys-cocoa`: macOS, then iOS
- [ ] `tpt-window-sys-x11` (x11rb)
- [ ] `tpt-window-sys-wayland` (sctk, stable xdg-shell; verify version first)
- [ ] IME implementation (all platforms)
- [ ] DPI / Retina / fractional scaling across all platforms
- [ ] Verify wayland-client/sctk licences pass cargo-deny
- [ ] **Exit:** same example runs on all platforms

## Phase 4: Capability & Compositor (Weeks 9–12)
- [ ] Get tpt-shell repo link; read tpt-archon-bridge/kernel capability + IPC API, replace stubs
- [ ] `tpt-window-archon` bridge crate (optional feature); standalone fallback
- [ ] Verify signed input events (spoofing prevention)
- [ ] tpt-shell protocol in `protocol.rs`; shared-memory channel
- [ ] Zero-copy frame sharing via `SurfaceHandle`
- [ ] Capability-gated input routing tests

## Phase 5: Polish & Docs (Weeks 13–14)
- [ ] Rustdoc for all public items
- [ ] Migration guide from winit
- [ ] Benchmarks vs winit (throughput + latency)
- [ ] cargo-deny zero violations; cargo-semver-checks
- [ ] Publish to crates.io (dry-run first)

## Success Criteria
- [ ] Zero allocation on hot path (CI-enforced)
- [ ] Caller owns main loop
- [ ] ECS-native window state
- [ ] AI-agent yield points exposed and tested
- [ ] Capability-aware (tpt-archon)
- [ ] Compositor-aware (tpt-shell)
- [ ] cargo-deny passes
- [ ] Windows, macOS, Linux (X11 + Wayland), Web all work
- [ ] Matches or exceeds winit throughput/latency
- [ ] tpt-nova, tpt-appfront, tpt-shell use tpt-window

## Open Risks
- tpt-archon capability/IPC API is undocumented; tpt-visual and tpt-shell links unknown
- Weeks 5–8 for three hard platforms is aggressive
- `windows-rs` compile times
