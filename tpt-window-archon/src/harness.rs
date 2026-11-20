// SPDX-License-Identifier: MIT OR Apache-2.0
//! An agent harness on yield points.
//!
//! An agent (an automated tester, a bot, an AI model driving an application) sees a window one
//! frame at a time. Each [`AgentFrame`] holds the events the loop delivered during that frame,
//! already checked against the router's signatures, and a snapshot of the window's state. The
//! agent answers by sending input through the same router, so its input is signed like any
//! other input and can be told apart from a person's.
//!
//! The harness does not decide anything for the agent. It only routes events, verifies them and
//! cuts them into frames at the yield points, so the agent sees the same ordering the loop did.
//!
//! ```ignore
//! let frames = harness.observe(&mut router, &state, loop_events)?;
//! for frame in frames {
//!     let reply = agent.decide(&frame);
//!     harness.act(&mut router, reply)?;
//! }
//! ```

use tpt_window_core::{Event, WindowEvent, WindowId, WindowState};

use crate::capability::Authority;
use crate::envelope::{EventVerifier, VerifyError};
use crate::router::{RouteError, WindowRouter};

/// Why the harness could not carry an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarnessError {
    /// The router refused a delivery or a receive (no capability, a full inbox, an unknown
    /// window).
    Route(RouteError),
    /// A routed event failed verification. Nothing the router sent is changed, so this means the
    /// signatures or the ordering are broken.
    Verify(VerifyError),
}

impl From<RouteError> for HarnessError {
    fn from(e: RouteError) -> Self {
        HarnessError::Route(e)
    }
}

impl From<VerifyError> for HarnessError {
    fn from(e: VerifyError) -> Self {
        HarnessError::Verify(e)
    }
}

impl core::fmt::Display for HarnessError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            HarnessError::Route(e) => write!(f, "routing: {e}"),
            HarnessError::Verify(e) => write!(f, "verification: {e}"),
        }
    }
}

impl std::error::Error for HarnessError {}

/// What the agent sees at one yield point.
#[derive(Clone, Debug, PartialEq)]
pub struct AgentFrame {
    /// The frame number the loop reported at the yield point that ends this frame.
    pub frame_number: u64,
    /// The events delivered during the frame, verified, in order. Yield points are not included.
    pub events: Vec<WindowEvent>,
    /// The window's state at the end of the frame.
    pub snapshot: WindowState,
}

/// Connects one agent to one window through a router.
pub struct AgentHarness<A: Authority> {
    window: WindowId,
    /// Capabilities the agent presents to receive the window's events (read on its channel).
    owner: Vec<A::Token>,
    /// Capabilities the agent presents to send input (write on its channel).
    source: Vec<A::Token>,
    verifier: EventVerifier,
}

impl<A: Authority> AgentHarness<A> {
    /// A harness for `window`. `verifier` must hold the router's public key.
    pub fn new(
        window: WindowId,
        owner: Vec<A::Token>,
        source: Vec<A::Token>,
        verifier: EventVerifier,
    ) -> Self {
        Self {
            window,
            owner,
            source,
            verifier,
        }
    }

    /// Routes the loop's events for this window into the agent's inbox, then reads them back
    /// verified and cut into frames. `events` are what the loop delivered (yield points
    /// included); events for other windows are ignored. `snapshot` is the window's state now.
    pub fn observe<I>(
        &mut self,
        router: &mut WindowRouter<A>,
        snapshot: &WindowState,
        events: I,
    ) -> Result<Vec<AgentFrame>, HarnessError>
    where
        I: IntoIterator<Item = Event>,
    {
        for ev in events {
            if ev.window != self.window {
                continue;
            }
            // The router signs what it stores. The source here is the loop itself, which holds
            // write rights on the window's channel.
            router.deliver(&self.source, self.window, ev.event)?;
        }
        let signed = router.receive(&self.owner, self.window)?;

        let mut frames = Vec::new();
        let mut current = Vec::new();
        for signed_event in &signed {
            let event = self.verifier.verify(signed_event)?;
            match event {
                WindowEvent::YieldPoint { frame_number, .. } => {
                    frames.push(AgentFrame {
                        frame_number,
                        events: std::mem::take(&mut current),
                        snapshot: *snapshot,
                    });
                }
                other => current.push(other),
            }
        }
        // Events after the last yield point wait for the next one; they are not a frame yet.
        Ok(frames)
    }

    /// Sends the agent's input for this window through the router. The router signs it, and the
    /// signature shows it came from the agent's source capability.
    pub fn act(
        &mut self,
        router: &mut WindowRouter<A>,
        event: WindowEvent,
    ) -> Result<(), HarnessError> {
        router.deliver(&self.source, self.window, event)?;
        Ok(())
    }
}
