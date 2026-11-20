// SPDX-License-Identifier: MIT OR Apache-2.0
//! The window reports the system's light or dark appearance, matching the registry value read
//! directly here.
#![cfg(windows)]

use tpt_window_core::*;
use tpt_window_sys_win32::new_event_loop;
use windows_sys::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};

#[test]
fn window_theme_matches_the_registry_value() {
    let el = new_event_loop().expect("event loop");
    let w = WindowBuilder::new()
        .with_title("tpt-window theme test")
        .build(&el)
        .expect("window");

    let key: Vec<u16> = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    let name: Vec<u16> = "AppsUseLightTheme".encode_utf16().chain(Some(0)).collect();
    let mut value: u32 = 0;
    let mut size = 4_u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            key.as_ptr(),
            name.as_ptr(),
            RRF_RT_REG_DWORD,
            core::ptr::null_mut(),
            (&mut value as *mut u32).cast(),
            &mut size,
        )
    };
    if status != 0 {
        eprintln!("skipped: AppsUseLightTheme is not set on this machine");
        return;
    }
    let expected = if value == 0 {
        Theme::Dark
    } else {
        Theme::Light
    };
    assert_eq!(w.theme(), Some(expected));
}
