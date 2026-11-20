// SPDX-License-Identifier: MIT OR Apache-2.0
//! The agent harness cuts routed events into frames at the yield points, verifies them, and
//! refuses an agent that lacks a capability.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use ed25519_dalek::SigningKey;
use tpt_window_archon::capability::{Authority, Resource, Right};
use tpt_window_archon::harness::{AgentHarness, HarnessError};
use tpt_window_archon::{EventSigner, EventVerifier, RouteError, WindowRouter};
use tpt_window_core::{
    CapabilityToken, DisplayScope, Event, InputScope, WindowEvent, WindowId, WindowScope,
    WindowState,
};

#[derive(Clone, Copy, Debug)]
struct Cap {
    serial: u64,
    resource: Resource,
    right: Right,
}

#[derive(Default)]
struct Issuer {
    next: u64,
    live: BTreeSet<u64>,
}

/// A shared issuer in the style of archon's `CapabilityIssuer`: a token works while it is live.
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Issuer>>);

impl Shared {
    fn mint(&self, resource: Resource, right: Right) -> Cap {
        let mut issuer = self.0.borrow_mut();
        let serial = issuer.next;
        issuer.next += 1;
        issuer.live.insert(serial);
        Cap {
            serial,
            resource,
            right,
        }
    }
    fn revoke(&self, cap: &Cap) {
        self.0.borrow_mut().live.remove(&cap.serial);
    }
}

impl Authority for Shared {
    type Token = Cap;

    fn authorizes(&self, token: &Cap, resource: Resource, right: Right) -> bool {
        if !self.0.borrow().live.contains(&token.serial) || token.resource != resource {
            return false;
        }
        let allows = |r: Right, read: bool| match r {
            Right::Read => read,
            Right::Write => !read,
            Right::ReadWrite => true,
        };
        match right {
            Right::Read => allows(token.right, true),
            Right::Write => allows(token.right, false),
            Right::ReadWrite => token.right == Right::ReadWrite,
        }
    }
}

fn scope() -> CapabilityToken {
    CapabilityToken {
        window_scope: WindowScope::Own,
        input_scope: InputScope::WindowOnly,
        display_scope: DisplayScope::Current,
    }
}

fn state() -> WindowState {
    WindowState::default()
}

fn setup(
    window: WindowId,
) -> (
    Shared,
    WindowRouter<Shared>,
    Vec<Cap>,
    Vec<Cap>,
    EventVerifier,
) {
    let issuer = Shared::default();
    let key = SigningKey::from_bytes(&[9; 32]);
    let verifier = EventVerifier::new(key.verifying_key());
    let mut router = WindowRouter::new(issuer.clone(), EventSigner::new(key));
    let owner = issuer.mint(Resource::Channel(window.0), Right::Read);
    router
        .open_window(&[owner], window, scope())
        .expect("window opens");
    let loop_source = issuer.mint(Resource::Channel(window.0), Right::Write);
    let agent_owner = issuer.mint(Resource::Channel(window.0), Right::Read);
    (
        issuer,
        router,
        vec![loop_source],
        vec![agent_owner],
        verifier,
    )
}

fn ev(window: WindowId, event: WindowEvent) -> Event {
    Event { window, event }
}

fn yield_at(frame: u64) -> WindowEvent {
    WindowEvent::YieldPoint {
        frame_number: frame,
        timestamp_ns: 0,
    }
}

#[test]
fn events_are_cut_into_frames_at_the_yield_points() {
    let window = WindowId(3);
    let (issuer, mut router, source, owner, verifier) = setup(window);
    let agent_source = issuer.mint(Resource::Channel(window.0), Right::Write);
    let mut harness = AgentHarness::new(window, owner, vec![source[0], agent_source], verifier);

    let batch = vec![
        ev(window, WindowEvent::Focused(true)),
        ev(window, WindowEvent::CursorEntered),
        ev(window, yield_at(0)),
        ev(window, WindowEvent::CursorLeft),
        ev(window, yield_at(1)),
    ];
    let frames = harness
        .observe(&mut router, &state(), batch)
        .expect("observe");

    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].frame_number, 0);
    assert_eq!(
        frames[0].events,
        vec![WindowEvent::Focused(true), WindowEvent::CursorEntered]
    );
    assert_eq!(frames[1].frame_number, 1);
    assert_eq!(frames[1].events, vec![WindowEvent::CursorLeft]);
}

#[test]
fn events_after_the_last_yield_wait_for_the_next_frame() {
    let window = WindowId(4);
    let (_issuer, mut router, source, owner, verifier) = setup(window);
    let mut harness = AgentHarness::<Shared>::new(window, owner, source, verifier);
    let frames = harness
        .observe(
            &mut router,
            &state(),
            vec![ev(window, WindowEvent::Focused(false))],
        )
        .expect("observe");
    assert!(frames.is_empty(), "no yield yet, so no frame");
}

#[test]
fn another_windows_events_are_ignored() {
    let window = WindowId(5);
    let (_issuer, mut router, source, owner, verifier) = setup(window);
    let mut harness = AgentHarness::<Shared>::new(window, owner, source, verifier);
    let frames = harness
        .observe(
            &mut router,
            &state(),
            vec![
                ev(WindowId(99), WindowEvent::Focused(true)),
                ev(window, yield_at(0)),
            ],
        )
        .expect("observe");
    assert_eq!(frames.len(), 1);
    assert!(frames[0].events.is_empty());
}

#[test]
fn an_agent_without_write_rights_cannot_send_input() {
    let window = WindowId(6);
    let (issuer, mut router, _source, owner, verifier) = setup(window);
    // A read-only capability: the agent can watch but not act.
    let watcher = issuer.mint(Resource::Channel(window.0), Right::Read);
    let mut harness = AgentHarness::new(window, owner, vec![watcher], verifier);
    let err = harness
        .act(&mut router, WindowEvent::Focused(true))
        .unwrap_err();
    assert_eq!(err, HarnessError::Route(RouteError::Denied));
}

#[test]
fn a_revoked_agent_is_refused_at_once() {
    let window = WindowId(7);
    let (issuer, mut router, _source, owner, verifier) = setup(window);
    let agent = issuer.mint(Resource::Channel(window.0), Right::Write);
    let mut harness = AgentHarness::new(window, owner, vec![agent], verifier);
    harness
        .act(&mut router, WindowEvent::Focused(true))
        .expect("allowed while live");
    issuer.revoke(&agent);
    assert_eq!(
        harness.act(&mut router, WindowEvent::Focused(false)),
        Err(HarnessError::Route(RouteError::Denied))
    );
}
