// SPDX-License-Identifier: MIT OR Apache-2.0
//! Capability-gated routing and signed delivery, end to end through the router.
//!
//! The authority here mirrors archon's `CapabilityIssuer`: a token authorizes only while it
//! is live (minted and not revoked), and a `ReadWrite` token counts for both reads and
//! writes. The real issuer is in tpt-archon, which this crate does not depend on.

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;

use ed25519_dalek::SigningKey;
use tpt_window_archon::capability::{Authority, DISPLAY_DEVICE, INPUT_DEVICE, Resource, Right};
use tpt_window_archon::{EventSigner, EventVerifier, RouteError, WindowRouter};
use tpt_window_core::{
    CapabilityToken, DisplayScope, ElementState, InputScope, KeyCode, PhysicalKey, WindowEvent,
    WindowId, WindowScope,
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

impl Issuer {
    fn mint(&mut self, resource: Resource, right: Right) -> Cap {
        let serial = self.next;
        self.next += 1;
        self.live.insert(serial);
        Cap {
            serial,
            resource,
            right,
        }
    }

    fn revoke(&mut self, cap: &Cap) {
        self.live.remove(&cap.serial);
    }
}

/// The shared issuer. It is cloned into the router and kept by the test to mint and revoke.
#[derive(Clone, Default)]
struct Shared(Rc<RefCell<Issuer>>);

impl Shared {
    fn mint(&self, resource: Resource, right: Right) -> Cap {
        self.0.borrow_mut().mint(resource, right)
    }
    fn revoke(&self, cap: &Cap) {
        self.0.borrow_mut().revoke(cap);
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

fn seed() -> SigningKey {
    SigningKey::from_bytes(&[42u8; 32])
}

fn plain_scope() -> CapabilityToken {
    CapabilityToken {
        window_scope: WindowScope::Own,
        input_scope: InputScope::WindowOnly,
        display_scope: DisplayScope::Current,
    }
}

fn key_press(key: KeyCode) -> WindowEvent {
    WindowEvent::KeyboardInput {
        state: ElementState::Pressed,
        key_code: key,
        physical_key: PhysicalKey::Code(key),
    }
}

fn setup() -> (Shared, WindowRouter<Shared>) {
    let issuer = Shared::default();
    let router = WindowRouter::new(issuer.clone(), EventSigner::new(seed()));
    (issuer, router)
}

#[test]
fn owner_receives_verified_events_from_an_authorized_source() {
    let (issuer, mut router) = setup();
    let win = WindowId(1);
    router.open_window(win, plain_scope()).unwrap();

    let source = issuer.mint(Resource::Channel(win.0), Right::Write);
    let owner = issuer.mint(Resource::Channel(win.0), Right::Read);
    router
        .deliver(&[source], win, key_press(KeyCode::A))
        .unwrap();
    router
        .deliver(&[source], win, key_press(KeyCode::B))
        .unwrap();

    let signed = router.receive(&[owner], win).unwrap();
    assert_eq!(signed.len(), 2);

    let mut verifier = EventVerifier::new(router.verifying_key());
    assert_eq!(verifier.verify(&signed[0]), Ok(key_press(KeyCode::A)));
    assert_eq!(verifier.verify(&signed[1]), Ok(key_press(KeyCode::B)));
    // Drained: the inbox is empty after receive.
    assert!(router.receive(&[owner], win).unwrap().is_empty());
}

#[test]
fn delivery_needs_a_write_capability_on_the_window_channel() {
    let (issuer, mut router) = setup();
    let win = WindowId(1);
    router.open_window(win, plain_scope()).unwrap();

    let read_only = issuer.mint(Resource::Channel(win.0), Right::Read);
    assert_eq!(
        router.deliver(&[read_only], win, key_press(KeyCode::A)),
        Err(RouteError::Denied)
    );
    // A write capability for a different window does not help.
    let other = issuer.mint(Resource::Channel(2), Right::Write);
    assert_eq!(
        router.deliver(&[other], win, key_press(KeyCode::A)),
        Err(RouteError::Denied)
    );
    // Nothing was queued by the failed attempts.
    let owner = issuer.mint(Resource::Channel(win.0), Right::Read);
    assert!(router.receive(&[owner], win).unwrap().is_empty());
}

#[test]
fn receive_needs_a_read_capability_on_the_window_channel() {
    let (issuer, mut router) = setup();
    let win = WindowId(1);
    router.open_window(win, plain_scope()).unwrap();
    let source = issuer.mint(Resource::Channel(win.0), Right::Write);
    router
        .deliver(&[source], win, key_press(KeyCode::A))
        .unwrap();

    let write_only = issuer.mint(Resource::Channel(win.0), Right::Write);
    assert_eq!(router.receive(&[write_only], win), Err(RouteError::Denied));
    let read_other = issuer.mint(Resource::Channel(9), Right::Read);
    assert_eq!(router.receive(&[read_other], win), Err(RouteError::Denied));
    assert_eq!(router.receive(&[], win), Err(RouteError::Denied));
}

#[test]
fn revoked_capabilities_stop_working_at_both_ends() {
    let (issuer, mut router) = setup();
    let win = WindowId(4);
    router.open_window(win, plain_scope()).unwrap();

    let source = issuer.mint(Resource::Channel(win.0), Right::Write);
    let owner = issuer.mint(Resource::Channel(win.0), Right::Read);
    router
        .deliver(&[source], win, key_press(KeyCode::A))
        .unwrap();

    issuer.revoke(&source);
    assert_eq!(
        router.deliver(&[source], win, key_press(KeyCode::B)),
        Err(RouteError::Denied)
    );

    issuer.revoke(&owner);
    assert_eq!(router.receive(&[owner], win), Err(RouteError::Denied));
}

#[test]
fn global_input_needs_the_input_device_read_as_well() {
    let (issuer, mut router) = setup();
    let win = WindowId(7);
    let scope = CapabilityToken {
        input_scope: InputScope::Global,
        ..plain_scope()
    };
    router.open_window(win, scope).unwrap();

    let owner = issuer.mint(Resource::Channel(win.0), Right::Read);
    assert_eq!(
        router.receive(&[owner], win),
        Err(RouteError::Denied),
        "owning the window is not enough for global input"
    );

    let input_device = issuer.mint(Resource::Device(INPUT_DEVICE), Right::Read);
    assert!(router.receive(&[owner, input_device], win).is_ok());
}

#[test]
fn any_display_needs_the_display_device_read() {
    let (issuer, mut router) = setup();
    let win = WindowId(8);
    let scope = CapabilityToken {
        display_scope: DisplayScope::Any,
        ..plain_scope()
    };
    router.open_window(win, scope).unwrap();

    let owner = issuer.mint(Resource::Channel(win.0), Right::Read);
    assert_eq!(router.receive(&[owner], win), Err(RouteError::Denied));
    let display = issuer.mint(Resource::Device(DISPLAY_DEVICE), Right::Read);
    assert!(router.receive(&[owner, display], win).is_ok());
}

#[test]
fn unroutable_scopes_are_rejected_when_the_window_opens() {
    let (_issuer, mut router) = setup();
    let scope = CapabilityToken {
        input_scope: InputScope::WindowAndChildren,
        ..plain_scope()
    };
    assert!(matches!(
        router.open_window(WindowId(1), scope),
        Err(RouteError::Scope(_))
    ));
    let scope = CapabilityToken {
        window_scope: WindowScope::OwnAndChildren,
        ..plain_scope()
    };
    assert!(matches!(
        router.open_window(WindowId(1), scope),
        Err(RouteError::Scope(_))
    ));
}

#[test]
fn unknown_and_closed_windows_are_errors() {
    let (issuer, mut router) = setup();
    let win = WindowId(3);
    let source = issuer.mint(Resource::Channel(win.0), Right::Write);
    let owner = issuer.mint(Resource::Channel(win.0), Right::Read);

    assert_eq!(
        router.deliver(&[source], win, key_press(KeyCode::A)),
        Err(RouteError::NoSuchWindow(win))
    );

    router.open_window(win, plain_scope()).unwrap();
    router
        .deliver(&[source], win, key_press(KeyCode::A))
        .unwrap();
    router.close_window(win);
    assert_eq!(
        router.receive(&[owner], win),
        Err(RouteError::NoSuchWindow(win))
    );
}

#[test]
fn an_event_cannot_be_moved_to_another_window() {
    // Spoofing across windows: an app that can read window A copies a genuine signed event
    // and presents it as input for window B. The signature covers the window id, so it fails.
    let (issuer, mut router) = setup();
    let (a, b) = (WindowId(10), WindowId(11));
    router.open_window(a, plain_scope()).unwrap();
    router.open_window(b, plain_scope()).unwrap();

    let src_a = issuer.mint(Resource::Channel(a.0), Right::Write);
    let owner_a = issuer.mint(Resource::Channel(a.0), Right::Read);
    router.deliver(&[src_a], a, key_press(KeyCode::Y)).unwrap();
    let mut from_a = router.receive(&[owner_a], a).unwrap();
    let mut moved = from_a.pop().unwrap();
    moved.window = b;

    let mut verifier_b = EventVerifier::new(router.verifying_key());
    assert!(verifier_b.verify(&moved).is_err());
}
