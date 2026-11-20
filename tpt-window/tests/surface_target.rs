// SPDX-License-Identifier: MIT OR Apache-2.0
//! `SurfaceTarget` gives renderers the window and display handles through the standard traits.
#![cfg(windows)]

use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use tpt_window::SurfaceTarget;
use tpt_window::prelude::*;

#[test]
fn surface_target_exposes_win32_handles() {
    let el = new_event_loop().expect("event loop");
    let w = WindowBuilder::new()
        .with_title("tpt-window surface target test")
        .build(&el)
        .expect("window");
    let target = SurfaceTarget::new(&w);

    let wh = target.window_handle().expect("window handle");
    assert!(matches!(wh.as_raw(), RawWindowHandle::Win32(_)));
    let dh = target.display_handle().expect("display handle");
    assert!(matches!(dh.as_raw(), RawDisplayHandle::Windows(_)));
}
