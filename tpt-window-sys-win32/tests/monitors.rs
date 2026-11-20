// SPDX-License-Identifier: MIT OR Apache-2.0
//! The display list matches the system: at least one display, exactly one primary, and sizes
//! and scale factors that make sense.
#![cfg(windows)]

use tpt_window_sys_win32::new_event_loop;

#[test]
fn lists_the_attached_displays() {
    let el = new_event_loop().expect("event loop");
    let monitors = el.available_monitors();
    assert!(!monitors.is_empty(), "no displays reported");
    assert_eq!(
        monitors.iter().filter(|m| m.primary).count(),
        1,
        "exactly one primary display: {monitors:?}"
    );
    for m in &monitors {
        assert!(
            m.size.width > 0 && m.size.height > 0,
            "empty display: {m:?}"
        );
        assert!(m.scale_factor >= 1.0, "scale below 1.0: {m:?}");
        assert!(m.name.is_some(), "display without a name: {m:?}");
    }
    let primary = monitors.iter().find(|m| m.primary).unwrap();
    assert_eq!(
        (primary.position.x, primary.position.y),
        (0, 0),
        "primary sits at the origin"
    );
}

#[test]
fn a_window_reports_the_display_it_is_on() {
    use tpt_window_core::*;
    let el = new_event_loop().expect("event loop");
    let w = WindowBuilder::new()
        .with_title("tpt-window current monitor test")
        .with_position(LogicalPosition::new(10.0, 10.0))
        .build(&el)
        .expect("window");
    let current = w.current_monitor().expect("a window on a display");
    assert!(
        el.available_monitors().contains(&current),
        "current monitor {current:?} is not in the display list"
    );
}

#[test]
fn a_window_placed_on_a_display_takes_that_displays_scale() {
    use tpt_window_core::*;
    let el = new_event_loop().expect("event loop");
    let primary = el
        .available_monitors()
        .into_iter()
        .find(|m| m.primary)
        .expect("a primary display");
    // The primary display's origin is (0, 0), so a window placed there opens on it.
    let w = WindowBuilder::new()
        .with_title("tpt-window initial dpi test")
        .with_position(LogicalPosition::new(0.0, 0.0))
        .build(&el)
        .expect("window");
    assert!(
        (w.scale_factor() - primary.scale_factor).abs() < 1e-9,
        "window scale {} but primary display scale {}",
        w.scale_factor(),
        primary.scale_factor
    );
}
