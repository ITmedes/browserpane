use super::bitrate::{compute_adapted_bitrate, DatagramStats};
use super::policy::viewer_can_forward_frame;
use super::request::{extract_token, validate_request_path, RequestValidationError};
use super::*;
use crate::auth::{AuthError, TokenValidator};
use bpane_protocol::frame::Frame;
use bpane_protocol::{ControlMessage, SessionFlags};

#[test]
fn extract_token_from_path() {
    assert_eq!(
        extract_token("/session?token=abc123"),
        Some("abc123".to_string())
    );
    assert_eq!(
        extract_token("/?token=xyz&other=1"),
        Some("xyz".to_string())
    );
    assert_eq!(extract_token("/session"), None);
    assert_eq!(extract_token("/session?other=1"), None);
}

#[test]
fn validate_request_path_accepts_valid_token() {
    let validator = TokenValidator::new(b"transport-request-secret".to_vec());
    let token = validator.generate_token();
    let path = format!("/session?token={token}");

    assert_eq!(validate_request_path(&path, &validator), Ok(()));
}

#[test]
fn validate_request_path_rejects_missing_token() {
    let validator = TokenValidator::new(b"transport-request-secret".to_vec());

    assert_eq!(
        validate_request_path("/session?other=1", &validator),
        Err(RequestValidationError::MissingToken)
    );
}

#[test]
fn validate_request_path_rejects_invalid_token() {
    let validator = TokenValidator::new(b"transport-request-secret".to_vec());

    assert_eq!(
        validate_request_path("/session?token=not-a-token", &validator),
        Err(RequestValidationError::InvalidToken(
            AuthError::MalformedToken
        ))
    );
}

#[test]
fn datagram_stats_initial_zero() {
    let stats = DatagramStats::new();
    let (s, f) = stats.take_counts();
    assert_eq!(s, 0);
    assert_eq!(f, 0);
}

#[test]
fn datagram_stats_counts_success_and_failure() {
    let stats = DatagramStats::new();
    stats.record_success();
    stats.record_success();
    stats.record_success();
    stats.record_failure();
    let (s, f) = stats.take_counts();
    assert_eq!(s, 3);
    assert_eq!(f, 1);
}

#[test]
fn datagram_stats_take_resets_counters() {
    let stats = DatagramStats::new();
    stats.record_success();
    stats.record_failure();
    let (s, f) = stats.take_counts();
    assert_eq!(s, 1);
    assert_eq!(f, 1);

    let (s2, f2) = stats.take_counts();
    assert_eq!(s2, 0);
    assert_eq!(f2, 0);
}

#[test]
fn datagram_stats_concurrent_access() {
    use std::sync::Arc;

    let stats = Arc::new(DatagramStats::new());
    let mut handles = Vec::new();
    for _ in 0..10 {
        let stats = Arc::clone(&stats);
        handles.push(std::thread::spawn(move || {
            for _ in 0..100 {
                stats.record_success();
                stats.record_failure();
            }
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let (s, f) = stats.take_counts();
    assert_eq!(s, 1000);
    assert_eq!(f, 1000);
}

#[test]
fn bitrate_adapts_down_on_high_failure() {
    let result = compute_adapted_bitrate(2_000_000, 80, 20);
    assert_eq!(result, 1_600_000);
}

#[test]
fn bitrate_adapts_down_on_moderate_failure() {
    let result = compute_adapted_bitrate(2_000_000, 95, 5);
    assert_eq!(result, 1_900_000);
}

#[test]
fn bitrate_adapts_up_on_zero_failure() {
    let result = compute_adapted_bitrate(2_000_000, 100, 0);
    assert_eq!(result, 2_100_000);
}

#[test]
fn bitrate_stays_same_on_low_failure() {
    let result = compute_adapted_bitrate(2_000_000, 99, 1);
    assert_eq!(result, 2_000_000);
}

#[test]
fn bitrate_clamps_to_minimum() {
    let mut bps = 300_000u32;
    for _ in 0..10 {
        bps = compute_adapted_bitrate(bps, 5, 50);
    }
    assert!(bps >= 200_000);
}

#[test]
fn bitrate_clamps_to_maximum() {
    let mut bps = 7_500_000u32;
    for _ in 0..50 {
        bps = compute_adapted_bitrate(bps, 100, 0);
    }
    assert!(bps <= 8_000_000);
}

#[test]
fn bitrate_no_change_on_zero_traffic() {
    let result = compute_adapted_bitrate(2_000_000, 0, 0);
    assert_eq!(result, 2_000_000);
}

#[test]
fn adapt_frame_for_client_strips_viewer_only_capabilities() {
    let frame = ControlMessage::SessionReady {
        version: 1,
        flags: SessionFlags::AUDIO
            | SessionFlags::CLIPBOARD
            | SessionFlags::FILE_TRANSFER
            | SessionFlags::MICROPHONE
            | SessionFlags::CAMERA
            | SessionFlags::KEYBOARD_LAYOUT,
    }
    .to_frame();

    let adapted = adapt_frame_for_client(&frame, false);

    assert_eq!(adapted.payload[0], 0x03);
    assert_ne!(adapted.payload[2] & SessionFlags::AUDIO.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::CLIPBOARD.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::FILE_TRANSFER.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::MICROPHONE.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::CAMERA.bits(), 0);
    assert_eq!(adapted.payload[2] & SessionFlags::KEYBOARD_LAYOUT.bits(), 0);
}

#[test]
fn adapt_frame_for_client_leaves_owner_flags_unchanged() {
    let frame = ControlMessage::SessionReady {
        version: 1,
        flags: SessionFlags::FILE_TRANSFER | SessionFlags::CAMERA,
    }
    .to_frame();

    let adapted = adapt_frame_for_client(&frame, true);
    assert_eq!(adapted, frame);
}

#[test]
fn viewer_can_receive_frame_blocks_clipboard_and_download() {
    let clipboard = Frame::new(ChannelId::Clipboard, vec![0x01]);
    let download = Frame::new(ChannelId::FileDown, vec![0x01]);
    let video = Frame::new(ChannelId::Video, vec![0x00]);

    assert!(!viewer_can_receive_frame(&clipboard));
    assert!(!viewer_can_receive_frame(&download));
    assert!(viewer_can_receive_frame(&video));
}

#[test]
fn viewer_can_forward_frame_blocks_interactive_channels() {
    let input = Frame::new(ChannelId::Input, vec![0x01]);
    let clipboard = Frame::new(ChannelId::Clipboard, vec![0x01]);
    let audio_in = Frame::new(ChannelId::AudioIn, vec![0x01]);
    let video_in = Frame::new(ChannelId::VideoIn, vec![0x01]);
    let file_up = Frame::new(ChannelId::FileUp, vec![0x01]);
    let layout = Frame::new(ChannelId::Control, vec![0x06, 0x00]);
    let pong = Frame::new(ChannelId::Control, vec![0x05]);

    assert!(!viewer_can_forward_frame(&input));
    assert!(!viewer_can_forward_frame(&clipboard));
    assert!(!viewer_can_forward_frame(&audio_in));
    assert!(!viewer_can_forward_frame(&video_in));
    assert!(!viewer_can_forward_frame(&file_up));
    assert!(!viewer_can_forward_frame(&layout));
    assert!(viewer_can_forward_frame(&pong));
}
