# Changelog

All notable changes to this crate are documented in this file.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
this crate follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Before
1.0.0, breaking changes may appear in minor releases.

## [Unreleased]

### Added
- Capability mapping (`capability`): window scopes onto archon's channel and device
  resources. Child-window scopes are rejected.
- Signed input events (`envelope`): Ed25519, with per-window sequence numbers against
  replay and reordering.
- `WindowRouter` (`router`): capability-gated delivery and receive over a generic `Authority`.
- `harness` module: a headless harness that checks capability routing and revocation end to end.
- `session_log` module: a signed session log with verification. The verifier is a fuzz target.
- Envelopes encode events with `tpt-window-core`'s codec instead of a private encoder.
- Signing keys are zeroized when dropped.
- Tests: routing and harness integration tests against a mock authority.
- Depends on `ed25519-dalek` 3. The session-log fuzz target uses the same version.
