use alloc::vec;
use alloc::vec::Vec;

use crate::{AudioFrame, ChannelId};

use super::super::{Frame, FrameError, Message};

#[test]
fn audio_frame_round_trip() {
    let frame = AudioFrame {
        seq: 100,
        timestamp_us: 2_000_000,
        data: vec![0x01, 0x02, 0x03],
    };
    assert_eq!(frame, AudioFrame::decode(&frame.encode()).unwrap());
}

#[test]
fn audio_frame_empty_data() {
    let frame = AudioFrame {
        seq: 0,
        timestamp_us: 0,
        data: Vec::new(),
    };
    assert_eq!(frame, AudioFrame::decode(&frame.encode()).unwrap());
}

#[test]
fn audio_frame_to_frame_out_channel() {
    let audio = AudioFrame {
        seq: 1,
        timestamp_us: 20_000,
        data: vec![0xAB; 16],
    };
    let frame = audio.to_frame_out();
    assert_eq!(frame.channel, ChannelId::AudioOut);
    assert_eq!(AudioFrame::decode(&frame.payload).unwrap(), audio);
}

#[test]
fn audio_frame_to_frame_out_message_dispatch() {
    let audio = AudioFrame {
        seq: 42,
        timestamp_us: 840_000,
        data: vec![0x00; 3840],
    };
    match Message::from_frame(&audio.to_frame_out()).unwrap() {
        Message::AudioOut(payload) => assert_eq!(AudioFrame::decode(&payload).unwrap(), audio),
        other => panic!("expected AudioOut, got {other:?}"),
    }
}

#[test]
fn audio_frame_pcm_20ms_round_trip() {
    let pcm_data = vec![0x7F; 3840];
    let audio = AudioFrame {
        seq: 100,
        timestamp_us: 2_000_000,
        data: pcm_data.clone(),
    };
    let wire = audio.to_frame_out().encode();
    let (decoded_frame, consumed) = Frame::decode(&wire).unwrap();
    assert_eq!(consumed, wire.len());
    assert_eq!(decoded_frame.channel, ChannelId::AudioOut);
    let decoded_audio = AudioFrame::decode(&decoded_frame.payload).unwrap();
    assert_eq!(decoded_audio.seq, 100);
    assert_eq!(decoded_audio.data, pcm_data);
}

#[test]
fn audio_frame_decode_truncated_header() {
    assert!(matches!(
        AudioFrame::decode(&[0u8; 10]),
        Err(FrameError::BufferTooShort { .. })
    ));
}

#[test]
fn audio_frame_decode_truncated_data() {
    let audio = AudioFrame {
        seq: 1,
        timestamp_us: 20_000,
        data: vec![0xAA; 100],
    };
    let encoded = audio.encode();
    assert!(matches!(
        AudioFrame::decode(&encoded[..encoded.len() - 50]),
        Err(FrameError::BufferTooShort { .. })
    ));
}

#[test]
fn audio_frame_trailing_data() {
    let mut encoded = AudioFrame {
        seq: 1,
        timestamp_us: 20_000,
        data: vec![0xBB; 10],
    }
    .encode();
    encoded.push(0xFF);
    assert!(matches!(
        AudioFrame::decode(&encoded),
        Err(FrameError::TrailingData(1))
    ));
}

#[test]
fn audio_frame_sequence_monotonic() {
    let frames: Vec<AudioFrame> = (0..5u32)
        .map(|i| AudioFrame {
            seq: (u32::MAX - 2).wrapping_add(i),
            timestamp_us: i as u64 * 20_000,
            data: vec![i as u8; 3840],
        })
        .collect();

    for frame in &frames {
        assert_eq!(AudioFrame::decode(&frame.encode()).unwrap().seq, frame.seq);
    }
    assert_eq!(frames[0].seq, u32::MAX - 2);
    assert_eq!(frames[1].seq, u32::MAX - 1);
    assert_eq!(frames[2].seq, u32::MAX);
    assert_eq!(frames[3].seq, 0);
    assert_eq!(frames[4].seq, 1);
}
