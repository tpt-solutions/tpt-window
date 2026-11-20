// SPDX-License-Identifier: MIT OR Apache-2.0
//! The display protocol between two processes over shared memory: a client and a server.
//!
//! The test runs its own binary a second time as the server (environment variable
//! `TPT_SHM_SERVER` carries the mapping names). Two rings carry the traffic: client to server
//! and server to client. The client opens a session with `Hello`, the server answers with
//! `Welcome`, and then each `FramePresented` is echoed back. `CloseWindow` ends the session.
#![cfg(windows)]

use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use tpt_window_core::channel::{FrameRing, RingError};
use tpt_window_core::protocol::{self, Message, PROTOCOL_VERSION};
use tpt_window_core::{SurfaceHandle, WindowId};
use tpt_window_shm::SharedMemory;

const SIZE: usize = 64 * 1024;
const FRAMES: u64 = 2_000;
const SERVER_ENV: &str = "TPT_SHM_SERVER";

fn frame(i: u64) -> Message {
    Message::FramePresented {
        window: WindowId(7),
        surface: SurfaceHandle { id: 3 },
        frame: i,
    }
}

fn push_blocking(ring: &mut FrameRing<SharedMemory>, msg: &Message, what: &str) {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match ring.push(msg) {
            Ok(()) => return,
            Err(RingError::Full) if Instant::now() < deadline => thread::yield_now(),
            Err(e) => panic!("{what}: push failed: {e}"),
        }
    }
}

fn pop_blocking(ring: &mut FrameRing<SharedMemory>, what: &str) -> Message {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        match ring.pop().unwrap_or_else(|e| panic!("{what}: {e}")) {
            Some(m) => return m,
            None if Instant::now() < deadline => thread::yield_now(),
            None => panic!("{what}: no message within 20 s"),
        }
    }
}

/// Server role: runs in the child process. Answers the client until `CloseWindow`.
fn run_server(names: &str) {
    let (to_server, to_client) = names.split_once('|').expect("two names");
    let mut inbox = FrameRing::new(SharedMemory::open(to_server, SIZE).expect("open inbox"))
        .expect("inbox size");
    let mut outbox = FrameRing::new(SharedMemory::open(to_client, SIZE).expect("open outbox"))
        .expect("outbox size");
    let mut echoed = 0u64;
    loop {
        match pop_blocking(&mut inbox, "server") {
            Message::Hello { version } => {
                let agreed = protocol::negotiate(version, PROTOCOL_VERSION)
                    .expect("client and server speak the same version");
                push_blocking(
                    &mut outbox,
                    &Message::Welcome { version: agreed },
                    "welcome",
                );
            }
            m @ Message::FramePresented { .. } => {
                push_blocking(&mut outbox, &m, "echo");
                echoed += 1;
            }
            Message::CloseWindow { .. } => break,
            other => panic!("server: unexpected {other:?}"),
        }
    }
    assert_eq!(echoed, FRAMES, "server echoed every frame");
}

#[test]
fn client_and_server_in_separate_processes_exchange_frames() {
    if let Ok(names) = std::env::var(SERVER_ENV) {
        run_server(&names);
        return;
    }

    let pid = std::process::id();
    let to_server = format!("tpt-window-shm-xproc-in-{pid}");
    let to_client = format!("tpt-window-shm-xproc-out-{pid}");
    let mut inbox = FrameRing::new(SharedMemory::create(&to_server, SIZE).expect("create inbox"))
        .expect("size");
    inbox.reset();
    let mut outbox = FrameRing::new(SharedMemory::create(&to_client, SIZE).expect("create outbox"))
        .expect("size");
    outbox.reset();

    // Run this same test binary as the server, selecting only this test.
    let exe = std::env::current_exe().expect("own path");
    let mut server = Command::new(exe)
        .args([
            "--exact",
            "client_and_server_in_separate_processes_exchange_frames",
            "--nocapture",
        ])
        .env(SERVER_ENV, format!("{to_server}|{to_client}"))
        .spawn()
        .expect("spawn server process");

    push_blocking(
        &mut inbox,
        &Message::Hello {
            version: PROTOCOL_VERSION,
        },
        "hello",
    );
    match pop_blocking(&mut outbox, "client") {
        Message::Welcome { version } => assert_eq!(version, PROTOCOL_VERSION),
        other => panic!("expected Welcome, got {other:?}"),
    }

    // Send frames and check the echoes come back in order. Sending and receiving interleave,
    // so neither ring can fill up and stall the other side.
    let mut sent = 0u64;
    let mut received = 0u64;
    while received < FRAMES {
        if sent < FRAMES && inbox.push(&frame(sent)).is_ok() {
            sent += 1;
        }
        if let Some(m) = outbox.pop().expect("valid echo") {
            assert_eq!(m, frame(received), "echo {received} out of order");
            received += 1;
        }
    }
    push_blocking(
        &mut inbox,
        &Message::CloseWindow {
            window: WindowId(7),
        },
        "close",
    );

    let status = server.wait().expect("server exits");
    assert!(status.success(), "server process failed: {status}");
}
