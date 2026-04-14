use tokio::sync::mpsc;

use bpane_protocol::frame::Message;
use bpane_protocol::{ClipboardMessage, ControlMessage, SessionFlags};

use crate::capture;

#[tokio::test]
async fn handle_control_ffmpeg_bitrate_hint() {
    let (tx, _rx) = mpsc::channel(16);
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();

    let msg = ControlMessage::BitrateHint {
        target_bps: 4_000_000,
    };
    super::handle_control_ffmpeg(msg, &tx, &cmd_tx).await;

    match cmd_rx.try_recv() {
        Ok(capture::ffmpeg::PipelineCmd::BitrateHint(bps)) => {
            assert_eq!(bps, 4_000_000);
        }
        other => panic!("expected BitrateHint, got {:?}", other.is_ok()),
    }
}

#[tokio::test]
async fn session_ready_includes_keyboard_layout_flag() {
    let flags = SessionFlags::new(
        SessionFlags::CLIPBOARD | SessionFlags::FILE_TRANSFER | SessionFlags::KEYBOARD_LAYOUT,
    );
    assert!(flags.has(SessionFlags::KEYBOARD_LAYOUT));
    assert!(flags.has(SessionFlags::CLIPBOARD));
    assert!(!flags.has(SessionFlags::AUDIO));
}

#[tokio::test]
async fn session_flags_with_audio() {
    let mut flags = SessionFlags::new(
        SessionFlags::CLIPBOARD | SessionFlags::FILE_TRANSFER | SessionFlags::KEYBOARD_LAYOUT,
    );
    flags = SessionFlags::new(flags.0 | SessionFlags::AUDIO | SessionFlags::MICROPHONE);
    assert!(flags.has(SessionFlags::AUDIO));
    assert!(flags.has(SessionFlags::MICROPHONE));
    assert!(flags.has(SessionFlags::CLIPBOARD));
}

#[tokio::test]
async fn session_flags_without_audio() {
    let flags = SessionFlags::new(
        SessionFlags::CLIPBOARD | SessionFlags::FILE_TRANSFER | SessionFlags::KEYBOARD_LAYOUT,
    );
    assert!(!flags.has(SessionFlags::AUDIO));
    assert!(!flags.has(SessionFlags::MICROPHONE));
}

#[tokio::test]
async fn session_ready_audio_flag_encodes_in_wire() {
    let flags = SessionFlags::new(
        SessionFlags::AUDIO
            | SessionFlags::CLIPBOARD
            | SessionFlags::FILE_TRANSFER
            | SessionFlags::KEYBOARD_LAYOUT,
    );
    let ready = ControlMessage::SessionReady { version: 2, flags };
    let frame = ready.to_frame();
    let decoded = ControlMessage::decode(&frame.payload).unwrap();
    match decoded {
        ControlMessage::SessionReady {
            version,
            flags: decoded_flags,
        } => {
            assert_eq!(version, 2);
            assert!(decoded_flags.has(SessionFlags::AUDIO));
            assert!(decoded_flags.has(SessionFlags::CLIPBOARD));
        }
        _ => panic!("expected SessionReady"),
    }
}

#[test]
fn has_audio_flag_extraction() {
    let with_audio = SessionFlags::new(
        SessionFlags::AUDIO | SessionFlags::CLIPBOARD | SessionFlags::KEYBOARD_LAYOUT,
    );
    assert!(with_audio.has(SessionFlags::AUDIO));

    let without_audio = SessionFlags::new(SessionFlags::CLIPBOARD | SessionFlags::KEYBOARD_LAYOUT);
    assert!(!without_audio.has(SessionFlags::AUDIO));
}

#[test]
fn clipboard_frame_dispatches_to_message_clipboard() {
    let msg = ClipboardMessage::Text {
        content: b"hello from browser".to_vec(),
    };
    let frame = msg.to_frame();
    let decoded = Message::from_frame(&frame).unwrap();
    match decoded {
        Message::Clipboard(ClipboardMessage::Text { content }) => {
            assert_eq!(content, b"hello from browser");
        }
        other => panic!("expected Message::Clipboard, got {:?}", other),
    }
}

#[test]
fn clipboard_frame_empty_text() {
    let msg = ClipboardMessage::Text {
        content: Vec::new(),
    };
    let frame = msg.to_frame();
    let decoded = Message::from_frame(&frame).unwrap();
    assert!(matches!(
        decoded,
        Message::Clipboard(ClipboardMessage::Text { content }) if content.is_empty()
    ));
}
