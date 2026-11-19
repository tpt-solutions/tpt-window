# tpt-window Todo

Tracks the phased rollout from [spec.txt](spec.txt). License: MIT OR Apache-2.0, TPT Solutions.

## Decisions (locked)
- **Ring buffer:** caller-owned, drop-newest. A full buffer returns `EventStatus::BufferFull` and bumps a drop counter. Drop-oldest is a consumer-side policy. `EventLoop` does not hold `Arc<RingBuffer>`. (Replaces the spec's "SPSC + drop oldest".)
- **`WindowEvent` is strictly `Copy`.** Text/IME use fixed inline buffers (e.g. 32-byte UTF-8, chunked when longer).
- **Multi-window** per `EventLoop`. **IME** deferred to Phase 3, API-ready now. **Clipboard** deferred to tpt-archon.
- **Spec correction:** "xdg-shell v5" is wrong. v5/v6 were unstable revisions, obsolete since wayland-protocols 1.12. Target stable `xdg_wm_base`, bind min(compositor, latest). Verify the version in Phase 3.
- **Sibling repos (found via the org page):** [tpt-archon](https://github.com/tpt-solutions/tpt-archon), [tpt-visual](https://github.com/tpt-solutions/tpt-visual) (facade crate `tpt-av-visual`), [tpt-nova](https://github.com/tpt-solutions/tpt-nova), [tpt-appfront](https://github.com/tpt-solutions/tpt-appfront), [tpt-shell](https://github.com/tpt-solutions/tpt-shell). Local stub types stay until the real APIs are matched.
- **Event API details:** `poll_events`/`step` take `&mut self`; `Window::raw_*_handle()` return `Result`; `YieldPoint` is emitted by `EventLoop`; the facade provides `new_event_loop()` instead of `EventLoop::new()`. See spec.txt section 12.
- **Event signing (archon bridge):** Ed25519 (`ed25519-dalek`, BSD-3-Clause, allowed by deny.toml). The router holds the secret key; applications verify with the public key only, so they cannot forge events. Each signature covers the window id, a per-window sequence number and a canonical encoding of the event. The sequence must increase, which rejects replay and reordering.
- **Capability mapping (archon bridge):** a window's events are `Resource::Channel(window id)`. The owner needs `Read` on it, an input source needs `Write`. `InputScope::Global` also needs `Read` on `Device(INPUT_DEVICE)`, and `DisplayScope::Any` needs `Read` on `Device(DISPLAY_DEVICE)`. `OwnAndChildren` and `WindowAndChildren` are rejected rather than granted, because child routing is not designed yet.
- **Key repeat:** a held key sends repeated `KeyboardInput::Pressed` events (text included), as on Win32. There is no `repeat` flag, so the event cannot tell a repeat from a new press. Implemented on X11 (detectable auto-repeat) and Wayland (sctk repeat timer).

## Needs your decision
- [x] **Decided: display-server role.** A new, separate crate fills it (decide its exact home at Phase 4; tpt-kernel / tpt-k-kernel not yet read closely). tpt-shell is a *client* of the versioned display protocol, not the compositor. Nothing before Phase 4 depends on it.
- [x] **Decided: archon capabilities.** Map window scopes onto archon's `Resource::Channel` / `Resource::Device` capabilities inside `tpt-window-archon`; do event signing there as an envelope (core stays crypto-free). Extend archon only if the mapping cannot express per-window scope.
- [x] Commit work to master (push is manual).
- [x] **Decided: IME surface.** Use the existing `ImeEvent::Preedit` (composition) and `ImeEvent::Commit` (committed text). Win32 first: composition is reported as Preedit from WM_IME_COMPOSITION; committed text arrives as WM_CHAR, the existing path. Cocoa, Wayland (text-input v3) and X11 (XIM) follow, each needing its own platform test.
- [x] **Decided: gamepads are not in the windowing layer.** Separate optional crate `tpt-window-gamepad` over `gilrs` 0.11. Own button, axis and event types; `poll` fills a caller-owned Vec and never blocks. Compile- and clippy-checked for Windows, Linux (needs libudev-dev, added to CI) and macOS; unit tests pass on Windows. Not tested with a real controller (none here).
- [x] **Decided: transparency belongs to the renderer.** The window layer cannot make a window see-through by itself (measured: DWM alone gives white). It needs a DXGI composition swapchain with composite alpha in the GPU code. Deferred to the renderer work.

## Phase 0: Repo & Governance
- [x] git init, .gitignore
- [x] LICENSE-MIT / LICENSE-APACHE (TPT Solutions, 2026), NOTICE
- [x] Workspace Cargo.toml, 11 members compile (facade, core, 5 backends, archon, test-utils, gamepad, bench)
- [x] deny.toml
- [x] CI workflow (fmt, clippy, test on win/linux/mac, wasm32 check, cargo-deny)
- [x] README, CONTRIBUTING (issues only, no PRs)
- [x] Verify `cargo deny check` passes locally (re-run after the bench and archon dependencies: passes)
- [x] GitHub repo `tpt-solutions/tpt-window` created and pushed
- [ ] Confirm CI green. The workflow fix (`main` → `master`) is committed locally, but not pushed. Needs a push, which is manual. Locally, every CI command passes on Windows and on WSL Ubuntu (see Verification).
- [x] Pin toolchain (rust-toolchain.toml: stable)
- [x] Confirm MSRV: `cargo +1.85 test --workspace --no-run` passes on Windows (all crates, bench and archon included).
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
- [x] Every public item documented: `KeyCode` variants and the DPI pair fields have docs now, with no `allow(missing_docs)` left. `RUSTDOCFLAGS="-D missing_docs -D rustdoc::broken_intra_doc_links" cargo doc --workspace` passes on Windows and Linux.
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
- [x] `protocol.rs`: wire format v1 (length-prefixed frames, tag per message, 4 KiB limit checked before any allocation), `Hello`/`Welcome` exact-version negotiation, and the surface and frame messages. Tested: round trips, truncated input, oversized length, unknown tags, bad payloads, version mismatch. The peer is still undecided, so there is no transport (see Phase 4).
- [x] `YieldPoint` emitted once per frame after OS events drained
- [x] `Window::set_ime_cursor_area` (command `SetImeCursorArea`), see Phase 3
### Backends
- [x] `tpt-window-test-utils`: headless backend + scripted event injection (5 tests)
- [x] `tpt-window-sys-wasm`: rendering verified in Edge (WebGPU) through tpt-nova. **Input and resize verified in Chromium** with a Playwright harness (`tpt-window-sys-wasm/browser-check`): pointer enter/move/click/wheel, key press/release, text commits, and canvas resize in both directions all arrive with the expected counts and no page errors. Not verified: Firefox and Safari. IME composition is not done (Phase 3). Earlier notes: `std::time::Instant` panics on wasm32 (core now uses `web-time`), and `wait()` cannot block in a browser, so the caller must drive `tick` from requestAnimationFrame. The earlier dump-dom runs were misleading: they cannot wait for GPU calls.
- [x] `tpt-window-sys-win32`: wndproc, per-monitor-v2 DPI, keyboard, mouse, wheel, text (WM_CHAR), focus, resize/move, min/max size, cross-thread setters. Verified against a real hidden window.
- [~] Win32 gaps: **touch** done for down/position (WM_TOUCH; verified with injected touch on this machine, `tests/touch.rs`). Lift (Ended) not verified: injected lifts time out here (ERROR_TIMEOUT). **raw mouse input** done: `WindowEvent::RawMouseMotion` from WM_INPUT, verified with SendInput (`tests/raw_input.rs`); one window per process receives it (the last created). Still open: pen, `transparent`, gamepads, IME composition with a real input method. 32-bit Windows is done: the Win32 test suite passes as an i686 build here, and CI has a job for it. Notes: pen cannot be tested by injection (Windows has no `InjectPenInput`); the pen design is `CursorMoved` with pressure from WM_POINTER. Transparency: extending the DWM frame into the client gives a white window, not a see-through one (measured with a screen capture), so it needs a DXGI composition swapchain with composite alpha on the renderer side.
- [x] Facade `tpt-window`: `new_event_loop()` selects win32 / wasm / X11 or Wayland / cocoa, `Unsupported` elsewhere
### Verification
- [x] `tpt-window-core/tests/rt_safety.rs`: allocation-counting allocator, 0 allocs on poll + push (plus a guard that the counter works)
- [x] Same check on the real Win32 pump: 2000 posted messages, 0 allocations (`tpt-window-sys-win32/tests/real_window.rs`). Both run in `cargo test --workspace`, which the CI matrix runs.
- [x] Examples: `hello_window` (Windows 88 frames in 1.5 s; X11 under Xvfb 127 frames in 2 s; Wayland under headless weston 126 frames in 2 s), `event_dump`
- [ ] **Exit:** Windows and Linux are done. Still open: macOS has not been run, and CI green on GitHub.

## Phase 2: Engine Integration (Weeks 3–4)
- [x] Found the repos. Findings so far: tpt-nova-app and tpt-nova-input depended on `winit = "0.30"`; tpt-appfront-canvas has its own internal Win32/X11/macOS window layer (windows-sys, objc2) that tpt-window could replace; tpt-visual uses wgpu behind the `tpt-av-visual` facade
- [x] Read tpt-nova-app's winit usage. The migration commit `9e229ab` (merged as `b124d72`) removed winit from `tpt-nova-app` (window creation, `ApplicationHandler`, close, resize, redraw, keyboard, modifiers, cursor, mouse buttons and wheel) and from `tpt-nova-input` and `tpt-nova-render`. Nova relied on `ModifiersChanged`, which tpt-window does not have, so the migration tracks modifier keys itself (see MIGRATION.md).
- [x] Read tpt-visual's surface-creation API: `tpt-av-visual/examples/simple_player.rs` builds the wgpu surface with `create_surface_unsafe(SurfaceTargetUnsafe::RawHandle { raw_display_handle, raw_window_handle })` from `window.raw_display_handle()` and `window.raw_window_handle()`.
- [x] `WindowResource` and `InputEventQueue` (feature `ecs` in core; compile-checked only, no dedicated test yet)
- [x] Replace winit in tpt-nova: merged into tpt-nova `master` (`b124d72`) and pushed. Nova now depends on tpt-window from GitHub (`0f9f849`), not a local path. WASM not done (nova has no WASM build yet). Known behaviour changes: shortcuts follow physical QWERTY positions; text comes from IME commits.
- [x] AI-agent yield point: the caller loop gets a `YieldPoint` once per step (`tpt-window-core`). Nova's loop ignores it for rendering; the display-sync test is still open.
- [x] Yield-point contract tested headless (`tpt-window-test-utils/tests/yield_sync.rs`): one yield per poll, last in its batch, numbered in order, lost yields counted in `dropped()` and visible as a frame-number gap. Not yet tested against real display refresh (vsync), which needs the nova or tpt-visual path.
- [x] tpt-visual: wgpu surface from raw handles; `Resized` reconfigures; frames render from the loop (not `RedrawRequested`). Done in `simple_player`, merged to tpt-visual local `master` (`b0af009`, not pushed). Runs for 12 s on a BGRA surface with no errors. The compositor's BGRA bug and the MSRV bump to 1.87 are fixed in that commit.
- [x] **Exit:** tpt-nova demo runs on tpt-window. Ran `tpt-nova-app` (debug build against tpt-window from GitHub, RTX 3050 on Windows): the "TPT Nova — Editor" window stays open for 10 s and renders its scene (screenshot checked). This is the GitHub revision, not the local working copy of tpt-window.

## Phase 3: Hard Platforms (Weeks 5–8)
- [~] `tpt-window-sys-cocoa`: written (AppKit via objc2: window, commands, NSEvent input, state diffs). **Added:** a window delegate whose `windowShouldClose:` vetoes the close button and reports `CloseRequested` (the app closes by dropping its window, as on Win32 and Wayland), and a tracking area on the content view for `CursorEntered`/`CursorLeft`. Compile- and clippy-checked for `aarch64-apple-darwin` on Windows; **not run on a Mac**. Still open: the CI test `tests/real_window.rs` has not been seen passing, and the delegate and tracking area are unverified at runtime.
- [~] `tpt-window-sys-x11`: written on Xlib loaded at runtime (`x11-dl`), not x11rb: wgpu's Xlib surface needs the `Display*`, which x11rb does not expose. Window, WM_DELETE_WINDOW close, input (keys with text, mouse, wheel), configure-driven resize and move. **Verified in WSL under Xvfb:** the lifecycle test passes. **Scale factor** comes from `Xft.dpi` (it was listed as a gap; it is implemented). **Key repeat** is done: detectable auto-repeat is enabled on open, and `tests/key_repeat.rs` drives a held key through XTest and gets 22 presses and 1 release. The same test fails with detectable repeat off (23 press/release pairs), so the check is real. Still open: IME.
- [~] `tpt-window-sys-wayland`: written (sctk 0.19: xdg toplevel, seat keyboard/pointer, configure, close request). **Verified in WSL against headless weston:** the lifecycle test passes. **Key repeat** is generated by sctk's repeat timer on the event loop (`get_keyboard_with_repeat`), and `key_pressed` is shared with the press path. Compile- and clippy-checked; **not exercised end to end**, because headless weston has no way to hold a key here. Gaps: SetSize/SetPosition/SetVisible cannot be expressed in Wayland, fractional scale (integer scale only), IME composition. Not yet tested with a real user session.
- [~] IME implementation: **Win32 written** (Preedit from WM_IME_COMPOSITION; Commit via the WM_CHAR path). **Candidate and composition placement** (`Window::set_ime_cursor_area` → `ImmSetCompositionWindow` + `ImmSetCandidateWindow`) is implemented and verified: `tests/ime_area.rs` reads the forms back through the window's IME context and checks the positions (the candidate list opens just below the area). Still open: verifying composition with a real Japanese or Chinese input method (none installed here, and the CI runners have none). Not started: Cocoa, Wayland (text-input v3), X11 (XIM). Each needs an input method server to test against.
- [ ] DPI / Retina / fractional scaling across all platforms. Done: Win32 per-monitor-v2, X11 `Xft.dpi`, Cocoa `backingScaleFactor` (not run on a Mac), web `devicePixelRatio`. Open: Wayland fractional scale (only integer `wl_output` scale is used), Retina verification on a Mac.
- [x] Verify wayland-client/sctk licences pass cargo-deny (checked locally; CI runs cargo-deny too)
- [ ] **Exit:** same example runs on all platforms. Done: `hello_window` runs on Windows, X11 and Wayland. Open: macOS (not run).

## Phase 4: Capability & Compositor (Weeks 9–12)
- [x] Read the archon sources the design depends on: `tpt-archon-bridge` `capability.rs` and `grant.rs`, `tpt-archon-kernel` `ipc.rs` and `driver.rs`. archon is `no_std` and its capability types use `Rc`, so they are not `Send`. Findings: a capability is an unforgeable `(Resource, Right)` token, revocation is checked at every enforcement point, and there is no signing, so signing lives in tpt-window's bridge.
- [x] `tpt-window-archon` bridge crate: the capability mapping (`capability`), signed events (`envelope`) and a capability-gated router (`router`). It does not depend on tpt-archon, so it works without archon (the standalone fallback). The `Authority` trait is what archon implements. **Open:** the adapter itself (`SharedIssuer` → `Authority`, a few lines) belongs in tpt-archon, which this change does not touch.
- [x] Verify signed input events (spoofing prevention): Ed25519, verified with the public key only. Tests: an altered event fails, an altered window id or sequence fails, an event signed by another key fails, a replayed or reordered event is rejected, and an event copied to another window fails. **Open:** the signing key is currently passed in by the caller; the tpt-archon key store that should hold it is not wired.
- [~] Display-server protocol in `protocol.rs`: **wire format v1 and version handshake done and tested** (in core, no I/O). **Shared-memory channel not done**: frame transport needs an OS mapping and atomics, which core cannot contain (`#![forbid(unsafe_code)]`), so it belongs in a platform crate. It also depends on the peer decision (tpt-shell is a webview shell, not a compositor).
- [ ] Zero-copy frame sharing via `SurfaceHandle`. Not started: `SurfaceHandle` is still a stub id. It needs the shared-memory channel above and the renderer side.
- [x] Capability-gated input routing tests: `tpt-window-archon/tests/routing.rs` (9 tests) runs the router against a mock authority that follows archon's `CapabilityIssuer` semantics (liveness on every check, `ReadWrite` covers both rights). Covers delivery and receive gating, revocation at both ends, global and any-display scopes, unroutable scopes rejected at open, unknown and closed windows, and cross-window spoofing.

## Phase 5: Polish & Docs (Weeks 13–14)
- [x] Rustdoc for all public items (see Phase 1: `missing_docs` and broken intra-doc links are denied and clean on Windows and Linux)
- [x] Migration guide from winit: [MIGRATION.md](MIGRATION.md), linked from the README. It maps every winit 0.30 event and window call, and lists the gaps: no `ModifiersChanged`, no file drop events, no IME cursor area on every platform, no cursor visibility or grab, no fullscreen, no transparency yet.
- [x] Benchmarks vs winit (throughput + latency), Windows: `tpt-window-bench`, one library per process, release build, `WM_MOUSEMOVE` posted to a hidden window, zero-timeout pump. tpt-window: 596 000–602 000 events/s (about 1.7 µs/event), p50 3.2–3.3 µs, p99 3.4–3.6 µs. winit 0.30: 392 000–396 000 events/s (about 2.5 µs/event), p50 4.3–4.4 µs, p99 4.6–5.0 µs. Two runs each. Only this scenario, on one machine. Linux and macOS not measured.
- [x] cargo-deny zero violations (advisories, bans, licences, sources pass). The bench is excluded from the shipped-dependency graph via `[graph] exclude-unpublished`, because winit's optional Wayland decorations pull in `ttf-parser`, which is flagged as unmaintained. The bench is not shipped.
- [x] cargo-semver-checks: 202 checks pass for each of the nine crates that exist at the baseline (`c26e9ee`, the initial commit). Nothing is published, so this only shows no breaking change against the scaffold. `tpt-window-gamepad` and the bench are not in that baseline.
- [ ] Publish to crates.io (dry-run first). **Skipped: publishing was excluded from this round by request.**

## Success Criteria
- [x] Zero allocation on hot path (CI-enforced): `rt_safety` (core) and the Win32 real-pump test both assert 0 allocations, and both run in `cargo test --workspace` on the CI matrix.
- [x] Caller owns main loop
- [ ] ECS-native window state (`WindowResource` / `InputEventQueue` compile, but there is no test and no engine uses them yet; nova uses the event queue directly)
- [x] AI-agent yield points exposed and tested
- [x] Capability-aware (tpt-archon): the bridge, its signing and its routing tests are in this repo. The tpt-archon adapter is still to be written in that repo.
- [ ] Compositor-aware (peer project to be decided, see Needs your decision)
- [x] cargo-deny passes
- [ ] Windows, macOS, Linux (X11 + Wayland), Web all work. Windows, Linux (X11 and Wayland) and web (Chromium) are verified. macOS has not been run.
- [~] Matches or exceeds winit throughput/latency: exceeds on Windows for the posted-message path (see benchmarks). Not measured on Linux, macOS or with real input devices.
- [ ] tpt-nova, tpt-appfront, tpt-shell use tpt-window. tpt-nova does (GitHub revision). tpt-appfront and tpt-shell have not been started.

## Open Risks
- tpt-archon has no window/input capability concept; tpt-shell is not a compositor (see Needs your decision). The bridge maps scopes onto the existing `Channel`/`Device` resources, so no archon change is needed for that. The tpt-archon adapter and the signing-key store still need writing there.
- WASM backend: verified in Chromium for input and resize. Firefox and Safari are not verified.
- Win32 real-window tests (`raw_input`, `real_window`) failed once in a full workspace run and passed on every later run. The cause was not found. If it returns, check for input focus or other windows on the desktop during the run.
- Cocoa, the Wayland key-repeat path and the Win32 IME composition with a real input method are compiled but not run end to end.
- `Window` getters take a `Mutex` per read; fine for now, revisit if benchmarks vs winit show it matters
- Weeks 5–8 for three hard platforms is aggressive
- `windows-rs` compile times
