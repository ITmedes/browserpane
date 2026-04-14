use alloc::{vec, vec::Vec};

use crate::{
    channel::ChannelId,
    frame::{Frame, FrameDecoder, FrameDecoderError},
};

#[test]
fn frame_decoder_reassembles_split_input() {
    let frame = Frame::new(ChannelId::Control, vec![1, 2, 3, 4]);
    let wire = frame.encode();
    let mut decoder = FrameDecoder::new();

    decoder.push(&wire[..2]).unwrap();
    assert_eq!(decoder.next_frame().unwrap(), None);

    decoder.push(&wire[2..]).unwrap();
    assert_eq!(decoder.next_frame().unwrap(), Some(frame));
    assert_eq!(decoder.next_frame().unwrap(), None);
}

#[test]
fn frame_decoder_waits_for_complete_payload_after_header() {
    let frame = Frame::new(ChannelId::Clipboard, vec![9, 8, 7, 6]);
    let wire = frame.encode();
    let mut decoder = FrameDecoder::new();

    decoder.push(&wire[..5]).unwrap();
    assert_eq!(decoder.next_frame().unwrap(), None);
    assert_eq!(decoder.pending_len(), 5);

    decoder.push(&wire[5..7]).unwrap();
    assert_eq!(decoder.next_frame().unwrap(), None);

    decoder.push(&wire[7..]).unwrap();
    assert_eq!(decoder.next_frame().unwrap(), Some(frame));
}

#[test]
fn frame_decoder_drains_multiple_frames() {
    let f1 = Frame::new(ChannelId::Input, vec![1]);
    let f2 = Frame::new(ChannelId::Cursor, vec![2, 3]);
    let mut wire = Vec::new();
    wire.extend_from_slice(&f1.encode());
    wire.extend_from_slice(&f2.encode());

    let mut decoder = FrameDecoder::new();
    decoder.push(&wire).unwrap();

    assert_eq!(decoder.drain_frames().unwrap(), vec![f1, f2]);
    assert_eq!(decoder.pending_len(), 0);
}

#[test]
fn frame_decoder_rejects_oversized_payload_headers() {
    let mut decoder = FrameDecoder::new();
    decoder.push(&[0x01, 0x01, 0x00, 0x00, 0x80]).unwrap();

    assert_eq!(
        decoder.next_frame(),
        Err(FrameDecoderError::Frame(
            crate::FrameError::PayloadTooLarge(2_147_483_649)
        ))
    );
}

#[test]
fn frame_decoder_rejects_unknown_channel_once_frame_is_complete() {
    let mut decoder = FrameDecoder::new();
    decoder.push(&[0xFF, 0x01, 0x00, 0x00, 0x00, 0xAA]).unwrap();

    assert_eq!(
        decoder.next_frame(),
        Err(FrameDecoderError::Frame(crate::FrameError::UnknownChannel(
            0xFF
        )))
    );
}

#[test]
fn frame_decoder_honors_pending_limit() {
    let frame = Frame::new(ChannelId::Control, vec![0; 8]);
    let wire = frame.encode();
    let mut decoder = FrameDecoder::with_max_pending(10);

    assert_eq!(
        decoder.push(&wire),
        Err(FrameDecoderError::PendingTooLarge {
            pending: wire.len(),
            max_pending: 10,
        })
    );
}

#[test]
fn frame_decoder_rejects_frame_larger_than_custom_limit_before_body_arrives() {
    let frame = Frame::new(ChannelId::Control, vec![0; 8]);
    let wire = frame.encode();
    let mut decoder = FrameDecoder::with_max_pending(12);

    decoder.push(&wire[..5]).unwrap();
    assert_eq!(
        decoder.next_frame(),
        Err(FrameDecoderError::PendingTooLarge {
            pending: wire.len(),
            max_pending: 12,
        })
    );
}
