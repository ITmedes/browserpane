use crate::{ChannelId, ControlMessage, SessionFlags};

use super::super::{FrameError, Message};

#[test]
fn control_resolution_request_round_trip() {
    let msg = ControlMessage::ResolutionRequest {
        width: 1920,
        height: 1080,
    };
    assert_eq!(msg, ControlMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn control_resolution_ack_round_trip() {
    let msg = ControlMessage::ResolutionAck {
        width: 800,
        height: 600,
    };
    assert_eq!(msg, ControlMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn control_session_ready_round_trip() {
    let msg = ControlMessage::SessionReady {
        version: 1,
        flags: SessionFlags::all(),
    };
    assert_eq!(msg, ControlMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn control_ping_pong_round_trip() {
    let ping = ControlMessage::Ping {
        seq: 42,
        timestamp_ms: 1_700_000_000_000,
    };
    let pong = ControlMessage::Pong {
        seq: 42,
        timestamp_ms: 1_700_000_000_005,
    };
    assert_eq!(ping, ControlMessage::decode(&ping.encode()).unwrap());
    assert_eq!(pong, ControlMessage::decode(&pong.encode()).unwrap());
}

#[test]
fn control_to_frame_round_trip() {
    let msg = ControlMessage::Ping {
        seq: 1,
        timestamp_ms: 999,
    };
    let frame = msg.to_frame();
    assert_eq!(frame.channel, ChannelId::Control);
    assert_eq!(Message::from_frame(&frame).unwrap(), Message::Control(msg));
}

#[test]
fn control_keyboard_layout_info_round_trip() {
    let mut layout_hint = [0u8; 32];
    layout_hint[..2].copy_from_slice(b"fr");
    let msg = ControlMessage::KeyboardLayoutInfo { layout_hint };
    let encoded = msg.encode();
    assert_eq!(encoded.len(), 33);
    assert_eq!(msg, ControlMessage::decode(&encoded).unwrap());
}

#[test]
fn control_keyboard_layout_info_empty() {
    let msg = ControlMessage::KeyboardLayoutInfo {
        layout_hint: [0u8; 32],
    };
    assert_eq!(msg, ControlMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn bitrate_hint_round_trip() {
    let msg = ControlMessage::BitrateHint {
        target_bps: 5_000_000,
    };
    assert_eq!(msg, ControlMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn bitrate_hint_dispatch_round_trip() {
    let msg = ControlMessage::BitrateHint {
        target_bps: 2_500_000,
    };
    let frame = msg.to_frame();
    assert_eq!(Message::from_frame(&frame).unwrap(), Message::Control(msg));
}

#[test]
fn bitrate_hint_bounds_round_trip() {
    for target_bps in [0, u32::MAX] {
        let msg = ControlMessage::BitrateHint { target_bps };
        assert_eq!(msg, ControlMessage::decode(&msg.encode()).unwrap());
    }
}

#[test]
fn control_unknown_tag() {
    assert!(matches!(
        ControlMessage::decode(&[0xFF]),
        Err(FrameError::UnknownMessageType { .. })
    ));
}

#[test]
fn control_trailing_data() {
    let mut encoded = ControlMessage::ResolutionAck {
        width: 100,
        height: 100,
    }
    .encode();
    encoded.push(0xFF);
    assert!(matches!(
        ControlMessage::decode(&encoded),
        Err(FrameError::TrailingData(1))
    ));
}

#[test]
fn control_truncated() {
    let encoded = ControlMessage::Ping {
        seq: 1,
        timestamp_ms: 100,
    }
    .encode();
    assert!(matches!(
        ControlMessage::decode(&encoded[..encoded.len() - 1]),
        Err(FrameError::BufferTooShort { .. })
    ));
}
