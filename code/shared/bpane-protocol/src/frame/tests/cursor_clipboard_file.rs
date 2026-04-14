use alloc::vec;
use alloc::vec::Vec;

use crate::{ChannelId, ClipboardMessage, CursorMessage, FileMessage};

#[test]
fn cursor_move_round_trip() {
    let msg = CursorMessage::CursorMove { x: 400, y: 300 };
    assert_eq!(msg, CursorMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn cursor_shape_round_trip() {
    let msg = CursorMessage::CursorShape {
        width: 32,
        height: 32,
        hotspot_x: 16,
        hotspot_y: 16,
        data: vec![0xFF; 32 * 32 * 4],
    };
    assert_eq!(msg, CursorMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn clipboard_text_round_trip() {
    let msg = ClipboardMessage::Text {
        content: b"Hello, clipboard!".to_vec(),
    };
    assert_eq!(msg, ClipboardMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn clipboard_empty_text() {
    let msg = ClipboardMessage::Text {
        content: Vec::new(),
    };
    assert_eq!(msg, ClipboardMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn file_header_round_trip() {
    let mut filename = [0u8; 256];
    filename[..13].copy_from_slice(b"test-file.txt");
    let mut mime = [0u8; 64];
    mime[..10].copy_from_slice(b"text/plain");
    let msg = FileMessage::header(1, filename, 1024, mime);
    assert_eq!(
        msg,
        FileMessage::decode_on_channel(&msg.encode(), ChannelId::FileDown).unwrap()
    );
}

#[test]
fn file_chunk_round_trip() {
    let msg = FileMessage::chunk(1, 0, vec![0xAB; 65_536]);
    assert_eq!(
        msg,
        FileMessage::decode_on_channel(&msg.encode(), ChannelId::FileDown).unwrap()
    );
}

#[test]
fn file_complete_round_trip() {
    let msg = FileMessage::complete(1);
    assert_eq!(
        msg,
        FileMessage::decode_on_channel(&msg.encode(), ChannelId::FileDown).unwrap()
    );
}

#[test]
fn file_decode_rejects_non_file_channel() {
    assert!(matches!(
        FileMessage::decode_on_channel(&FileMessage::complete(1).encode(), ChannelId::Control),
        Err(crate::frame::FrameError::InvalidFieldValue {
            field: "file channel",
            value
        }) if value == u64::from(ChannelId::Control.as_u8())
    ));
}
