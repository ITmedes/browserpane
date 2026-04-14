use alloc::vec;

use crate::{
    ChannelId, ClipboardMessage, ControlMessage, CursorMessage, FileMessage, InputMessage,
    Modifiers,
};

use super::super::{Frame, Message};

#[test]
fn message_dispatch_all_typed_channels() {
    let ctrl = ControlMessage::Ping {
        seq: 1,
        timestamp_ms: 100,
    };
    let input = InputMessage::MouseMove { x: 10, y: 20 };
    let input_key_ex = InputMessage::KeyEventEx {
        keycode: 16,
        down: true,
        modifiers: 0,
        key_char: 0x61,
    };
    let layout_info = ControlMessage::KeyboardLayoutInfo {
        layout_hint: [0u8; 32],
    };
    let cursor = CursorMessage::CursorMove { x: 5, y: 10 };
    let clipboard = ClipboardMessage::Text {
        content: b"hi".to_vec(),
    };
    let file = FileMessage::complete(99);

    assert!(matches!(
        Message::from_frame(&ctrl.to_frame()),
        Ok(Message::Control(_))
    ));
    assert!(matches!(
        Message::from_frame(&input.to_frame()),
        Ok(Message::Input(_))
    ));
    assert!(matches!(
        Message::from_frame(&input_key_ex.to_frame()),
        Ok(Message::Input(_))
    ));
    assert!(matches!(
        Message::from_frame(&layout_info.to_frame()),
        Ok(Message::Control(_))
    ));
    assert!(matches!(
        Message::from_frame(&cursor.to_frame()),
        Ok(Message::Cursor(_))
    ));
    assert!(matches!(
        Message::from_frame(&clipboard.to_frame()),
        Ok(Message::Clipboard(_))
    ));
    assert!(matches!(
        Message::from_frame(&file.to_frame(ChannelId::FileDown)),
        Ok(Message::FileDown(_))
    ));
    assert!(matches!(
        Message::from_frame(&file.to_frame(ChannelId::FileUp)),
        Ok(Message::FileUp(_))
    ));
}

#[test]
fn message_dispatch_key_event_ex_preserves_fields() {
    let msg = InputMessage::KeyEventEx {
        keycode: 18,
        down: true,
        modifiers: Modifiers::ALT,
        key_char: 0x20AC,
    };
    assert_eq!(
        Message::from_frame(&msg.to_frame()).unwrap(),
        Message::Input(msg)
    );
}

#[test]
fn message_dispatch_keyboard_layout_info_preserves_fields() {
    let mut layout_hint = [0u8; 32];
    layout_hint[..5].copy_from_slice(b"de-CH");
    let msg = ControlMessage::KeyboardLayoutInfo { layout_hint };
    assert_eq!(
        Message::from_frame(&msg.to_frame()).unwrap(),
        Message::Control(msg)
    );
}

#[test]
fn message_dispatch_raw_channels() {
    let video_frame = Frame::new(ChannelId::Video, vec![0x00, 0x00, 0x01]);
    assert!(matches!(
        Message::from_frame(&video_frame),
        Ok(Message::Video(_))
    ));

    let audio_frame = Frame::new(ChannelId::AudioOut, vec![0xAA]);
    assert!(matches!(
        Message::from_frame(&audio_frame),
        Ok(Message::AudioOut(_))
    ));
}
