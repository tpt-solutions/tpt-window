// SPDX-License-Identifier: MIT OR Apache-2.0
//! Every window event survives encoding and decoding, and the bytes stay fixed.

use tpt_window_core::codec::{
    CodecError, decode_event, decode_record, encode_event, encode_record,
};
use tpt_window_core::*;

fn samples() -> Vec<WindowEvent> {
    vec![
        WindowEvent::CloseRequested,
        WindowEvent::Destroyed,
        WindowEvent::Resized(PhysicalSize::new(640, 480)),
        WindowEvent::Moved(PhysicalPosition::new(-32000, 7)),
        WindowEvent::ScaleFactorChanged {
            scale_factor: 1.25,
            new_inner_size: PhysicalSize::new(800, 600),
        },
        WindowEvent::Focused(true),
        WindowEvent::Occluded(false),
        WindowEvent::KeyboardInput {
            state: ElementState::Pressed,
            key_code: KeyCode::F24,
            physical_key: PhysicalKey::Unidentified(0x5E),
        },
        WindowEvent::Ime(ImeEvent::Enabled),
        WindowEvent::Ime(ImeEvent::Preedit {
            text: ImeText::new("にほん"),
            continued: true,
        }),
        WindowEvent::Ime(ImeEvent::Commit {
            text: ImeText::new("é"),
            continued: false,
        }),
        WindowEvent::Ime(ImeEvent::Disabled),
        WindowEvent::CursorMoved {
            position: PhysicalPosition::new(12.5, -3.0),
            pressure: 0.5,
        },
        WindowEvent::CursorEntered,
        WindowEvent::CursorLeft,
        WindowEvent::MouseInput {
            state: ElementState::Released,
            button: MouseButton::Other(9),
        },
        WindowEvent::MouseWheel {
            delta: MouseScrollDelta::LineDelta { x: 0.0, y: -1.0 },
            phase: TouchPhase::Moved,
        },
        WindowEvent::MouseWheel {
            delta: MouseScrollDelta::PixelDelta { x: 2.0, y: 4.5 },
            phase: TouchPhase::Ended,
        },
        WindowEvent::Touch(TouchEvent {
            id: 42,
            phase: TouchPhase::Started,
            position: PhysicalPosition::new(1.0, 2.0),
            force: 0.25,
        }),
        WindowEvent::RawMouseMotion { dx: -5, dy: 9 },
        WindowEvent::GamepadConnected(GamepadId(3)),
        WindowEvent::GamepadDisconnected(GamepadId(3)),
        WindowEvent::RedrawRequested,
        WindowEvent::YieldPoint {
            frame_number: 12,
            timestamp_ns: 987_654,
        },
        WindowEvent::ModifiersChanged(ModifiersState::new(true, false, true, false)),
        WindowEvent::ThemeChanged(Theme::Dark),
        WindowEvent::User(u64::MAX),
        WindowEvent::FileHovered {
            chunk: ImeText::new("C:/a"),
            continued: false,
        },
        WindowEvent::FileDropped {
            chunk: ImeText::new("part"),
            continued: true,
        },
        WindowEvent::FileHoverCancelled,
    ]
}

#[test]
fn every_variant_round_trips() {
    for event in samples() {
        let mut bytes = Vec::new();
        encode_event(&event, &mut bytes);
        let (decoded, used) = decode_event(&bytes).unwrap_or_else(|e| panic!("{event:?}: {e}"));
        assert_eq!(decoded, event);
        assert_eq!(used, bytes.len(), "{event:?} used its whole encoding");
    }
}

#[test]
fn records_round_trip_with_their_window() {
    let mut bytes = Vec::new();
    encode_record(WindowId(77), &WindowEvent::Focused(true), &mut bytes);
    let (event, used) = decode_record(&bytes).expect("decodes");
    assert_eq!(event.window, WindowId(77));
    assert_eq!(event.event, WindowEvent::Focused(true));
    assert_eq!(used, bytes.len());
}

#[test]
fn the_byte_layout_is_fixed() {
    // These bytes are what signatures and recordings already contain. Changing them breaks
    // verification of earlier logs, so they are pinned here.
    let mut bytes = Vec::new();
    encode_event(&WindowEvent::Focused(true), &mut bytes);
    assert_eq!(bytes, [5, 1]);

    bytes.clear();
    encode_event(
        &WindowEvent::KeyboardInput {
            state: ElementState::Pressed,
            key_code: KeyCode::A,
            physical_key: PhysicalKey::Code(KeyCode::A),
        },
        &mut bytes,
    );
    assert_eq!(bytes, [7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);

    bytes.clear();
    encode_event(&WindowEvent::User(1), &mut bytes);
    assert_eq!(bytes, [66, 1, 0, 0, 0, 0, 0, 0, 0]);
}

#[test]
fn damaged_bytes_are_errors_not_panics() {
    assert_eq!(decode_event(&[]), Err(CodecError::Truncated));
    assert_eq!(decode_event(&[200]), Err(CodecError::BadTag(200)));
    // A bool byte other than 0 or 1.
    assert_eq!(decode_event(&[5, 7]), Err(CodecError::BadValue));
    // A key number nobody has.
    assert_eq!(
        decode_event(&[7, 0, 0xFE, 0xFF, 0xFF, 0x7F, 0, 0, 0, 0, 0]),
        Err(CodecError::BadValue)
    );
    // A path chunk longer than one ImeText cannot be one this encoder wrote.
    let mut long = vec![70];
    let text = "x".repeat(40);
    long.extend_from_slice(&(text.len() as u32).to_le_bytes());
    long.extend_from_slice(text.as_bytes());
    long.push(0);
    assert_eq!(decode_event(&long), Err(CodecError::BadValue));
}
