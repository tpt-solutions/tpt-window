# tpt-window Todo

Tracks the phased rollout from [spec.txt](spec.txt). License: MIT OR Apache-2.0, TPT Solutions.

## Decisions (locked)
- **Ring buffer:** caller-owned, drop-newest. A full buffer returns `EventStatus::BufferFull` and bumps a drop counter. Drop-oldest is a consumer-side policy. `EventLoop` does not hold `Arc<RingBuffer>`. (Replaces the spec's "SPSC + drop oldest".)
- **`WindowEvent` is strictly `Copy`.** Text/IME use fixed inline buffers (e.g. 32-byte UTF-8, chunked when longer).
- **Multi-window** per `EventLoop`. **IME** deferred to Phase 3, API-ready now. **Clipboard** deferred to tpt-archon.
- **Spec correction:** "xdg-shell v5" is wrong. v5/v6 were unstable revisions, obsolete since wayland-protocols 1.12. Target stable `xdg_wm_base`, bind min(compositor, latest). Verify the version in Phase 3.
- **Sibling repos (found via the org page):** [tpt-archon](https://github.com/tpt-solutions/tpt-archon), [tpt-visual](https://github.com/tpt-solutions/tpt-visual) (facade crate `tpt-av-visual`), [tpt-nova](https://github.com/tpt-solutions/tpt-nova), [tpt-appfront](https://github.com/tpt-solutions/tpt-appfront), [tpt-shell](https://github.com/tpt-solutions/tpt-shell). Local stub types stay until the real APIs are matched.
- **Event API details:** `poll_events`/`step` take `&mut self`; `Window::raw_*_handle()` return `Result`; `YieldPoint` is emitted by `EventLoop`; the facade provides `new_event_loop()` instead of `EventLoop::new()`. See spec.txt section 12.

## Needs your decision
- [x] **Decided: display-server role.** A new, separate crate fills it (decide its exact home at Phase 4; tpt-kernel / tpt-k-kernel not yet read closely). tpt-shell is a *client* of the versioned display protocol, not the compositor. Nothing before Phase 4 depends on it.
- [x] **Decided: archon capabilities.** Map window scopes onto archon's `Resource::Channel` / `Resource::Device` capabilities inside `tpt-window-archon`; do event signing there as an envelope (core stays crypto-free). Extend archon only if the mapping cannot express per-window scope.
- [x] Commit work to master (push is manual).

## Phase 0: Repo & Governance
- [x] git init, .gitignore
- [x] LICENSE-MIT / LICENSE-APACHE (TPT Solutions, 2026), NOTICE
- [x] Workspace Cargo.toml, 9 crates compile (facade, core, 5 backends, archon, test-utils)
- [x] deny.toml
- [x] CI workflow (fmt, clippy, test on win/linux/mac, wasm32 check, cargo-deny)
- [x] README, CONTRIBUTING (issues only, no PRs)
- [x] Verify `cargo deny check` passes locally
- [x] GitHub repo `tpt-solutions/tpt-window` created and pushed
- [ ] Confirm CI green (the workflow only triggered on `main` but the default branch is `master`; fixed locally, not pushed)
- [x] Pin toolchain (rust-toolchain.toml: stable)
- [ ] Confirm MSRV (set to 1.85, unverified; edition 2024 needs >= 1.85)
- [x] ~~CODE_OF_CONDUCT.md, SECURITY.md~~ (dropped by decision; CONTRIBUTING is issues-only)
- [x] Update spec.txt with the Decisions above (new section 12, plus fixes in 4.4, 4.5, 10)
- [x] First commit (c26e9ee)

## Phase 1: Core & Proof of Concept (Weeks 1–2)
### Core types (`tpt-window-core`)
- [x] `WindowError`, `WindowId`
- [x] `PhysicalSize/Position`, `LogicalSize/Position`
- [x] `KeyCode`, `PhysicalKey`, `ElementState`, `MouseButton`, `MouseScrollDelta`, `TouchPhase`, `TouchEvent`, `GamepadId`
- [x] `ImeEvent` with inline fixed-size text
- [x] `WindowEvent` (strictly `Copy`) + compile-time `Copy` and size (<= 64 B) assertion
- [x] `raw-window-handle` 0.6 re-exports
### Ring buffer / sink
- [x] `RingBuffer<T: Copy>`: fixed capacity, allocated once (default 1024)
- [x] `EventSink`: `push`, `captured_count`
- [x] `EventStatus` (`Accepted`, `BufferFull`, `Captured`)
- [x] Drop counter + configurable policy (only counts events actually lost)
- [x] Unit tests
- [x] Property tests for `RingBuffer` and `InlineText` (proptest, `tests/properties.rs`). Fuzzing not done.
### Traits and API
- [x] `EventLoopBackend`, `WindowBackend` traits
- [x] `EventLoop`: `with_backend`, `poll_events`, `step`, `request_exit` (multi-window)
- [x] `Window`: Arc handle, cached-snapshot getters (no syscalls), command-queue setters
- [x] `WindowBuilder`: all fields, `with_capability` stubbed
- [x] Stub `CapabilityToken`, `SurfaceHandle`, `WindowScope`/`InputScope`/`DisplayScope`
- [x] `protocol.rs` skeleton (version constant only; blocked on the compositor decision)
- [x] `YieldPoint` emitted once per frame after OS events drained
### Backends
- [x] `tpt-window-test-utils`: headless backend + scripted event injection (5 tests)
- [ ] `tpt-window-sys-wasm`: written (canvas, keyboard, mouse, wheel, touch, focus, ResizeObserver, rAF redraw); **compile- and clippy-checked only, never run in a browser**. Needs a wasm-bindgen-test / headless-browser run. No IME composition yet.
- [x] `tpt-window-sys-win32`: wndproc, per-monitor-v2 DPI, keyboard, mouse, wheel, text (WM_CHAR), focus, resize/move, min/max size, cross-thread setters. Verified against a real hidden window.
- [ ] Win32 gaps: touch/pen, `transparent`, raw input (WM_INPUT), gamepads, IME composition, 32-bit Windows (`GetWindowLongPtrW`)
- [x] Facade `tpt-window`: `new_event_loop()` selects win32 / wasm, `Unsupported` elsewhere
### Verification
- [x] `tpt-window-core/tests/rt_safety.rs`: allocation-counting allocator, 0 allocs on poll + push (plus a guard that the counter works)
- [x] Same check on the real Win32 pump: 2000 posted messages, 0 allocations (`tpt-window-sys-win32/tests/real_window.rs`)
- [x] Examples: `hello_window` (ran: 88 frames in 1.5 s), `event_dump`
- [ ] **Exit:** Windows is done. Still open: WASM verified in a browser; CI green on GitHub.

## Phase 2: Engine Integration (Weeks 3–4)
- [x] Found the repos. Findings so far: tpt-nova-app and tpt-nova-input depend on `winit = "0.30"`; tpt-appfront-canvas has its own internal Win32/X11/macOS window layer (windows-sys, objc2) that tpt-window could replace; tpt-visual uses wgpu behind the `tpt-av-visual` facade
- [ ] Read tpt-nova-app's winit usage in detail and tpt-visual's surface-creation API
- [x] `WindowResource` and `InputEventQueue` (feature `ecs` in core; compile-checked only, no dedicated test yet)
- [~] Replace winit in tpt-nova: **Windows done**, merged to tpt-nova local `master` (`b124d72`, not pushed; path deps to `../tpt-window`, switch to a published/git dep before pushing). Editor smoke-run opens a window and reaches its loop; not visually checked. WASM not done (nova has no WASM build yet). Known behaviour changes: shortcuts follow physical QWERTY positions; text comes from IME commits.
- [x] AI-agent yield point: the caller loop gets a `YieldPoint` once per step (`tpt-window-core`). Nova's loop ignores it for rendering; the display-sync test is still open.
- [ ] Test that display refresh stays in sync with the AI-agent yield point
- [x] tpt-visual: wgpu surface from raw handles; `Resized` reconfigures; frames render from the loop (not `RedrawRequested`). Done in `simple_player`, merged to tpt-visual local `master` (`b0af009`, not pushed). Runs for 12 s on a BGRA surface with no errors. The compositor's BGRA bug and the MSRV bump to 1.87 are fixed in that commit.
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
- [x] Read `tpt-archon-bridge::capability` (see Needs your decision). Still to read: `tpt-archon-kernel` `ipc.rs` and `driver.rs`, `tpt-archon-bridge` `grant.rs`; archon is `no_std` and its capability types use `Rc`, so they are not `Send`
- [ ] `tpt-window-archon` bridge crate (optional feature); standalone fallback
- [ ] Verify signed input events (spoofing prevention)
- [ ] Display-server protocol in `protocol.rs` and shared-memory channel (peer to be decided; tpt-shell is a webview shell)
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
- [ ] Compositor-aware (peer project to be decided, see Needs your decision)
- [ ] cargo-deny passes
- [ ] Windows, macOS, Linux (X11 + Wayland), Web all work
- [ ] Matches or exceeds winit throughput/latency
- [ ] tpt-nova, tpt-appfront, tpt-shell use tpt-window

## Open Risks
- tpt-archon has no window/input capability concept; tpt-shell is not a compositor (see Needs your decision)
- WASM backend unverified in a browser
- `Window` getters take a `Mutex` per read; fine for now, revisit if benchmarks vs winit show it matters
- Weeks 5–8 for three hard platforms is aggressive
- `windows-rs` compile times
