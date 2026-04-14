use alloc::vec;
use alloc::vec::Vec;

use crate::ChannelId;

use super::super::envelope::MAX_PAYLOAD_SIZE;
use super::super::{Frame, FrameError, FRAME_HEADER_SIZE};

#[test]
fn frame_round_trip() {
    let frame = Frame::new(ChannelId::Control, vec![1, 2, 3, 4]);
    let encoded = frame.encode();
    let (decoded, consumed) = Frame::decode(&encoded).unwrap();
    assert_eq!(frame, decoded);
    assert_eq!(consumed, encoded.len());
}

#[test]
fn frame_empty_payload() {
    let frame = Frame::new(ChannelId::Input, vec![]);
    let encoded = frame.encode();
    let (decoded, consumed) = Frame::decode(&encoded).unwrap();
    assert_eq!(frame, decoded);
    assert_eq!(consumed, FRAME_HEADER_SIZE);
}

#[test]
fn frame_decode_too_short() {
    assert!(matches!(
        Frame::decode(&[0x0A, 0x01]),
        Err(FrameError::BufferTooShort { .. })
    ));
}

#[test]
fn frame_decode_unknown_channel() {
    let buf = [0xFF, 0x00, 0x00, 0x00, 0x00];
    assert!(matches!(
        Frame::decode(&buf),
        Err(FrameError::UnknownChannel(0xFF))
    ));
}

#[test]
fn frame_decode_payload_too_large() {
    let buf = [0x0A, 0x00, 0x00, 0x00, 0x02];
    assert!(matches!(
        Frame::decode(&buf),
        Err(FrameError::PayloadTooLarge(_))
    ));
}

#[test]
fn frame_decode_all_multiple() {
    let f1 = Frame::new(ChannelId::Control, vec![1, 2]);
    let f2 = Frame::new(ChannelId::Input, vec![3, 4, 5]);
    let mut buf = Vec::from(f1.encode().as_ref());
    buf.extend_from_slice(&f2.encode());
    let (frames, consumed) = Frame::decode_all(&buf).unwrap();
    assert_eq!(frames, vec![f1, f2]);
    assert_eq!(consumed, buf.len());
}

#[test]
fn frame_decode_all_partial() {
    let frame = Frame::new(ChannelId::Control, vec![1, 2]);
    let mut buf = Vec::from(frame.encode().as_ref());
    buf.extend_from_slice(&[0x0A, 0x10]);
    let (frames, consumed) = Frame::decode_all(&buf).unwrap();
    assert_eq!(frames, vec![frame]);
    assert!(consumed < buf.len());
}

#[test]
#[should_panic(expected = "frame payload too large")]
fn frame_encode_panics_on_oversized_payload() {
    let payload = vec![0u8; MAX_PAYLOAD_SIZE as usize + 1];
    let frame = Frame::new(ChannelId::Control, payload);
    let _ = frame.encode();
}

#[test]
fn frame_encode_max_payload_succeeds() {
    let payload = vec![0u8; 1024];
    let frame = Frame::new(ChannelId::Control, payload);
    let encoded = frame.encode();
    assert_eq!(encoded.len(), FRAME_HEADER_SIZE + 1024);
}
