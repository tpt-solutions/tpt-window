// SPDX-License-Identifier: MIT OR Apache-2.0
//! Client-side display protocol (versioned).
//!
//! **Skeleton.** The spec names `tpt-shell` as the compositor peer, but tpt-shell is a
//! webview application shell, not a compositor. The peer for this protocol is an open
//! decision (see todo.md); until it is made only the version constant is defined.

/// Protocol version. Bumped on any wire-incompatible change.
pub const PROTOCOL_VERSION: u32 = 0;
