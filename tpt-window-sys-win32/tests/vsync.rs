// SPDX-License-Identifier: MIT OR Apache-2.0
//! Yield points against the display's refresh. The loop paces itself with `DwmFlush`, which
//! returns once the compositor has presented a frame, and checks that each frame yields
//! exactly once, with no gaps, at the display's rate. This is the check the contract tests
//! in `tpt-window-test-utils` could not make: they do not wait on a real display.
//! Skips when DWM composition is off, since `DwmFlush` then returns at once.
#![cfg(windows)]

use std::time::{Duration, Instant};

use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::Graphics::Dwm::{DwmFlush, DwmIsCompositionEnabled};
use windows_sys::core::BOOL;

const FRAMES: usize = 240;

#[test]
fn yield_points_follow_the_display_refresh_without_gaps() {
    let mut enabled: BOOL = 0;
    unsafe { DwmIsCompositionEnabled(&mut enabled) };
    if enabled == 0 {
        eprintln!("skipping: DWM composition is off");
        return;
    }

    let mut el = new_event_loop().expect("event loop");
    let mut q = EventQueue::with_capacity(256);
    let _w = WindowBuilder::new()
        .with_title("tpt-window vsync test")
        .with_inner_size(LogicalSize::new(320.0, 200.0))
        .build(&el)
        .expect("window");

    let mut yields: Vec<(u64, u64)> = Vec::with_capacity(FRAMES);
    let mut stamps: Vec<Instant> = Vec::with_capacity(FRAMES);
    let started = Instant::now();
    for _ in 0..FRAMES {
        // Zero timeout: the flush below paces the loop, not the event wait.
        el.step(&mut EventSink::new(&mut q), Duration::ZERO);
        for ev in q.drain() {
            if let WindowEvent::YieldPoint {
                frame_number,
                timestamp_ns,
            } = ev.event
            {
                yields.push((frame_number, timestamp_ns));
                stamps.push(Instant::now());
            }
        }
        // Wait for the compositor to present, as a renderer would after its frame.
        unsafe { DwmFlush() };
    }
    let elapsed = started.elapsed();

    // Exactly one yield per frame, numbered 0..FRAMES with no gap, so none was lost.
    assert_eq!(yields.len(), FRAMES, "one yield point per frame");
    for (i, (frame, _)) in yields.iter().enumerate() {
        assert_eq!(*frame, i as u64, "frame numbers must be contiguous");
    }
    assert_eq!(q.dropped(), 0, "the queue must not drop a yield point");

    // Timestamps are monotonic, and the average interval matches the display's refresh.
    assert!(yields.windows(2).all(|w| w[0].1 <= w[1].1));
    let per_frame = elapsed.as_secs_f64() / FRAMES as f64;
    eprintln!(
        "vsync: {FRAMES} frames in {:.3} s, {:.2} ms per frame ({:.1} Hz)",
        elapsed.as_secs_f64(),
        per_frame * 1e3,
        1.0 / per_frame
    );
    assert!(
        (0.004..0.060).contains(&per_frame),
        "average frame interval {per_frame:.4} s is not a display refresh"
    );
    let max_gap = stamps
        .windows(2)
        .map(|w| w[1].duration_since(w[0]))
        .max()
        .unwrap_or_default();
    // A shared CI runner can stall the compositor for a single frame, so allow a small
    // number of long gaps (at most 1% of frames). A sustained stall still fails.
    let long_gaps = stamps
        .windows(2)
        .filter(|w| w[1].duration_since(w[0]) >= Duration::from_millis(250))
        .count();
    eprintln!("vsync: longest frame gap {max_gap:?}, {long_gaps} gap(s) over 250 ms");
    assert!(
        long_gaps <= FRAMES / 100,
        "{long_gaps} frames stalled for 250 ms or more (longest {max_gap:?})"
    );
}
