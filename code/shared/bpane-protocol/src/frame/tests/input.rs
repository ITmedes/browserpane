use crate::{InputMessage, Modifiers, MouseButton};

use super::super::FrameError;

#[test]
fn input_mouse_move_round_trip() {
    let msg = InputMessage::MouseMove { x: 100, y: 200 };
    assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn input_mouse_button_round_trip() {
    let msg = InputMessage::MouseButton {
        button: MouseButton::Left,
        down: true,
        x: 50,
        y: 75,
    };
    assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn input_mouse_scroll_round_trip() {
    let msg = InputMessage::MouseScroll { dx: -3, dy: 5 };
    assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn input_key_event_round_trip() {
    let msg = InputMessage::KeyEvent {
        keycode: 0x001E,
        down: true,
        modifiers: Modifiers::CTRL | Modifiers::SHIFT,
    };
    assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn input_negative_scroll_values() {
    let msg = InputMessage::MouseScroll {
        dx: -32768,
        dy: 32767,
    };
    assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn input_key_event_ex_round_trip() {
    let msg = InputMessage::KeyEventEx {
        keycode: 30,
        down: true,
        modifiers: Modifiers::empty(),
        key_char: 0x61,
    };
    let encoded = msg.encode();
    assert_eq!(encoded.len(), 11);
    assert_eq!(msg, InputMessage::decode(&encoded).unwrap());
}

#[test]
fn input_key_event_ex_unicode_round_trip() {
    let msg = InputMessage::KeyEventEx {
        keycode: 3,
        down: true,
        modifiers: Modifiers::empty(),
        key_char: 0xE9,
    };
    assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn input_key_event_ex_non_printable_round_trip() {
    let msg = InputMessage::KeyEventEx {
        keycode: 1,
        down: true,
        modifiers: Modifiers::empty(),
        key_char: 0,
    };
    assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn input_key_event_ex_euro_sign() {
    let msg = InputMessage::KeyEventEx {
        keycode: 18,
        down: true,
        modifiers: Modifiers::ALT,
        key_char: 0x20AC,
    };
    assert_eq!(msg, InputMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn input_unknown_tag() {
    assert!(matches!(
        InputMessage::decode(&[0xFF]),
        Err(FrameError::UnknownMessageType { .. })
    ));
}

#[test]
fn input_invalid_mouse_button_value() {
    assert_eq!(
        InputMessage::decode(&[0x02, 0xFF, 0x01, 0x00, 0x00, 0x00, 0x00]),
        Err(FrameError::InvalidFieldValue {
            field: "mouse button",
            value: 0xFF,
        })
    );
}
