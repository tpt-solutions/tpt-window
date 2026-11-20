// SPDX-License-Identifier: MIT OR Apache-2.0
//! A proxy used from another thread wakes a blocked `step`, and can stop the loop.
#![cfg(windows)]

use std::time::{Duration, Instant};

use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;

#[test]
fn wake_from_another_thread_ends_a_long_wait_early() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(64);
    let _w = WindowBuilder::new()
        .with_title("tpt-window proxy test")
        .build(&el)
        .expect("window");
    let proxy = el.proxy();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        proxy.wake();
    });

    let started = Instant::now();
    el.step(&mut EventSink::new(&mut q), Duration::from_secs(10));
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "wake did not interrupt the wait: {:?}",
        started.elapsed()
    );
}

#[test]
fn request_exit_from_another_thread_stops_the_loop() {
    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(64);
    let _w = WindowBuilder::new()
        .with_title("tpt-window proxy exit test")
        .build(&el)
        .expect("window");
    let proxy = el.proxy();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        proxy.request_exit();
    });

    let started = Instant::now();
    let mut status = PollStatus::Idle;
    while status != PollStatus::Exit && started.elapsed() < Duration::from_secs(5) {
        status = el.step(&mut EventSink::new(&mut q), Duration::from_secs(1));
        q.drain().for_each(drop);
    }
    assert_eq!(status, PollStatus::Exit, "loop did not stop");
}
