// SPDX-License-Identifier: MIT OR Apache-2.0
//! Capability bridge between tpt-window and tpt-archon.
//!
//! Three pieces, each usable on its own:
//!
//! - [`capability`]: the mapping from tpt-window's scopes ([`CapabilityToken`]) onto
//!   archon's `(Resource, Right)` capabilities. Windows are channels, and the input and
//!   display devices are devices. Scopes the mapping cannot express yet are rejected.
//! - [`envelope`]: input events signed with Ed25519 by the trusted router. The
//!   application verifies them with the public key only, so it cannot forge events for
//!   another window. A per-window sequence number stops replays and reordering.
//! - [`router`]: the router itself. It checks capabilities on every delivery and
//!   receive, signs what it delivers, and keeps one inbox per window.
//!
//! The router is generic over an [`Authority`]. In tpt-archon that is a thin adapter over
//! `CapabilityIssuer::authorizes`. This crate does not depend on tpt-archon, so it builds
//! and runs without it. See the README for the status of the adapter.
//!
//! [`CapabilityToken`]: tpt_window_core::CapabilityToken
//! [`capability`]: crate::capability
//! [`envelope`]: crate::envelope
//! [`router`]: crate::router
//! [`Authority`]: crate::capability::Authority
#![forbid(unsafe_code)]
#![warn(missing_docs)]

pub mod capability;
pub mod envelope;
pub mod router;

pub use capability::{Authority, Resource, Right, ScopeError};
pub use envelope::{EventSigner, EventVerifier, SignedEvent, VerifyError};
pub use router::{RouteError, WindowRouter};
