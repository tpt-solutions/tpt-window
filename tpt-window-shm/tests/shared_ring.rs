// SPDX-License-Identifier: MIT OR Apache-2.0
//! A frame ring over a real shared mapping, with a producer and a consumer on separate
//! threads. Each side maps the same named memory separately, so the test exercises the
//! release/acquire ordering the ring depends on, not just a `Vec`.
#![cfg(windows)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use tpt_window_core::channel::{FrameRing, RingError};
use tpt_window_core::protocol::Message;
use tpt_window_core::{SurfaceHandle, WindowId};
use tpt_window_shm::{SharedMemory, ShmError};

/// Unique per test run, so parallel runs do not collide.
fn unique_name(tag: &str) -> String {
    static N: AtomicUsize = AtomicUsize::new(0);
    format!(
        "tpt-window-shm-test-{}-{}-{}",
        std::process::id(),
        tag,
        N.fetch_add(1, Ordering::Relaxed)
    )
}

fn frame(i: u64) -> Message {
    Message::FramePresented {
        window: WindowId(1 + i % 3),
        surface: SurfaceHandle { id: i % 5 },
        frame: i,
    }
}

const SIZE: usize = 64 * 1024;
const FRAMES: u64 = 50_000;

#[test]
fn producer_and_consumer_on_separate_views_see_every_frame_in_order() {
    let name = unique_name("spsc");
    let mem = SharedMemory::create(&name, SIZE).expect("create mapping");
    let mut producer = FrameRing::new(mem).expect("size");
    producer.reset();

    // The consumer maps the same name separately, as another process would.
    let consumer_mem = SharedMemory::open(&name, SIZE).expect("open mapping");
    let mut consumer = FrameRing::new(consumer_mem).expect("size");

    let writer = thread::spawn(move || {
        for i in 0..FRAMES {
            loop {
                match producer.push(&frame(i)) {
                    Ok(()) => break,
                    Err(RingError::Full) => thread::yield_now(),
                    Err(e) => panic!("push failed: {e}"),
                }
            }
        }
        producer
    });

    let mut next = 0u64;
    while next < FRAMES {
        match consumer.pop().expect("valid frames") {
            Some(m) => {
                assert_eq!(m, frame(next), "frame {next} out of order or corrupt");
                next += 1;
            }
            None => thread::yield_now(),
        }
    }
    let _producer = writer.join().expect("producer thread");
    assert_eq!(consumer.queued_bytes(), 0);
}

#[test]
fn a_name_cannot_be_created_twice_and_a_missing_name_is_reported() {
    let name = unique_name("dup");
    let _first = SharedMemory::create(&name, SIZE).expect("first create");
    assert_eq!(
        SharedMemory::create(&name, SIZE).err(),
        Some(ShmError::AlreadyExists)
    );
    assert_eq!(
        SharedMemory::open(&unique_name("missing"), SIZE).err(),
        Some(ShmError::NotFound)
    );
    assert_eq!(
        SharedMemory::create("", SIZE).err(),
        Some(ShmError::BadName)
    );
}

#[test]
fn a_mapping_too_small_for_one_frame_is_refused_by_the_ring() {
    let name = unique_name("tiny");
    let mem = SharedMemory::create(&name, 64).expect("create");
    assert!(matches!(
        FrameRing::new(mem),
        Err(RingError::TooSmall { .. })
    ));
}

#[test]
fn opening_with_a_larger_size_than_the_mapping_is_refused() {
    // The mapping is 64 KiB, so a view of 1 MiB must fail rather than map past the end.
    let name = unique_name("sized");
    let _owner = SharedMemory::create(&name, 64 * 1024).expect("create");
    assert!(
        SharedMemory::open(&name, 1024 * 1024).is_err(),
        "a view past the end of the mapping must be refused"
    );
    assert!(SharedMemory::open(&name, 64 * 1024).is_ok());
}
