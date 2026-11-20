// SPDX-License-Identifier: MIT OR Apache-2.0
//! Capability-gated, signed delivery of window events.
//!
//! The router is the trusted side. Input sources deliver events to windows, and
//! applications receive them. Each call checks a presented capability against the
//! [`Authority`] before doing anything. Delivered events are signed, so the application
//! can check origin with [`EventVerifier`](crate::envelope::EventVerifier).

use std::collections::BTreeMap;

use ed25519_dalek::VerifyingKey;
use tpt_window_core::{CapabilityToken, WindowEvent, WindowId};

use crate::capability::{self, Authority, Resource, Right, ScopeError};
use crate::envelope::{EventSigner, SignedEvent};

/// Events a window may have waiting for its owner. A full inbox refuses new events rather than
/// growing, so a source that floods a window cannot exhaust the router's memory.
pub const INBOX_CAPACITY: usize = 1024;

/// Why a routing call failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RouteError {
    /// No presented capability authorizes the required right.
    Denied,
    /// The window's inbox is full. Nothing was signed or stored; retry after the owner receives.
    InboxFull,
    /// The window is not registered with the router.
    NoSuchWindow(WindowId),
    /// The window's scope cannot be routed yet.
    Scope(ScopeError),
}

impl core::fmt::Display for RouteError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RouteError::Denied => f.write_str("no capability authorizes this"),
            RouteError::InboxFull => f.write_str("the window's inbox is full"),
            RouteError::NoSuchWindow(w) => write!(f, "window {} is not registered", w.0),
            RouteError::Scope(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for RouteError {}

/// Routes signed window events, gated by capabilities.
pub struct WindowRouter<A: Authority> {
    authority: A,
    signer: EventSigner,
    scopes: BTreeMap<WindowId, CapabilityToken>,
    inboxes: BTreeMap<WindowId, Vec<SignedEvent>>,
}

impl<A: Authority> WindowRouter<A> {
    /// Creates a router that checks capabilities with `authority` and signs with `signer`.
    pub fn new(authority: A, signer: EventSigner) -> Self {
        Self {
            authority,
            signer,
            scopes: BTreeMap::new(),
            inboxes: BTreeMap::new(),
        }
    }

    /// The public key that applications verify delivered events with.
    pub fn verifying_key(&self) -> VerifyingKey {
        self.signer.verifying_key()
    }

    /// Registers a window with its scope. `owner` must authorize reading the window's channel:
    /// only a party that was given the window may register it. Scopes that cannot be routed are
    /// rejected here, not silently downgraded.
    pub fn open_window(
        &mut self,
        owner: &[A::Token],
        window: WindowId,
        token: CapabilityToken,
    ) -> Result<(), RouteError> {
        if !self.any_authorizes(owner, Resource::Channel(window.0), Right::Read) {
            return Err(RouteError::Denied);
        }
        capability::requirements(&token, window).map_err(RouteError::Scope)?;
        self.scopes.insert(window, token);
        self.inboxes.entry(window).or_default();
        Ok(())
    }

    /// Unregisters a window and drops its pending events. The signer keeps the window's
    /// sequence counter, so if the id is registered again its events still increase and an
    /// old event cannot be replayed into the new window.
    pub fn close_window(&mut self, window: WindowId) {
        self.scopes.remove(&window);
        self.inboxes.remove(&window);
    }

    /// Delivers `event` to `window`. `source` is the capabilities the input source
    /// presents. One of them must authorize writing the window's channel.
    pub fn deliver(
        &mut self,
        source: &[A::Token],
        window: WindowId,
        event: WindowEvent,
    ) -> Result<(), RouteError> {
        if !self.scopes.contains_key(&window) {
            return Err(RouteError::NoSuchWindow(window));
        }
        if !self.any_authorizes(source, Resource::Channel(window.0), Right::Write) {
            return Err(RouteError::Denied);
        }
        // Check room before signing, so a refused event does not use up a sequence number.
        let full = self
            .inboxes
            .get(&window)
            .is_some_and(|inbox| inbox.len() >= INBOX_CAPACITY);
        if full {
            return Err(RouteError::InboxFull);
        }
        let signed = self.signer.sign(window, event);
        if let Some(inbox) = self.inboxes.get_mut(&window) {
            inbox.push(signed);
        }
        Ok(())
    }

    /// Takes the pending events for `window`. `caps` must together authorize everything the
    /// window's scope requires, which means reading its channel and, for global input or
    /// any-display placement, reading the input or display device.
    pub fn receive(
        &mut self,
        caps: &[A::Token],
        window: WindowId,
    ) -> Result<Vec<SignedEvent>, RouteError> {
        let token = *self
            .scopes
            .get(&window)
            .ok_or(RouteError::NoSuchWindow(window))?;
        let req = capability::requirements(&token, window).map_err(RouteError::Scope)?;
        for &(resource, right) in req.as_slice() {
            if !self.any_authorizes(caps, resource, right) {
                return Err(RouteError::Denied);
            }
        }
        Ok(self
            .inboxes
            .get_mut(&window)
            .map(std::mem::take)
            .unwrap_or_default())
    }

    fn any_authorizes(&self, caps: &[A::Token], resource: Resource, right: Right) -> bool {
        caps.iter()
            .any(|cap| self.authority.authorizes(cap, resource, right))
    }
}
