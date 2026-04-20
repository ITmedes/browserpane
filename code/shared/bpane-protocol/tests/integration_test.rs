//! Integration tests for the BrowserPane protocol pipeline.
//!
//! Tests the full frame encoding/decoding chain:
//! 1. Message creation -> binary encoding -> frame wrapping
//! 2. Frame transport simulation (write to buffer, read back)
//! 3. Frame decoding -> message extraction -> validation
//!
//! Also tests the video fragmentation/reassembly pipeline.

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::{Frame, Message, FRAME_HEADER_SIZE};
use bpane_protocol::*;

/// Simulate sending frames through a byte-oriented transport (like a Unix socket).
/// Writes multiple frames to a buffer, then reads them back and validates.
#[test]
fn full_pipeline_control_messages() {
    let mut layout_hint = [0u8; 32];
    layout_hint[..2].copy_from_slice(b"fr");

    let messages = vec![
        ControlMessage::SessionReady {
            version: 2,
            flags: SessionFlags::all(),
        },
        ControlMessage::ResolutionRequest {
            width: 1920,
            height: 1080,
        },
        ControlMessage::ResolutionAck {
            width: 1920,
            height: 1080,
        },
        ControlMessage::Ping {
            seq: 1,
            timestamp_ms: 1_700_000_000_000,
        },
        ControlMessage::Pong {
            seq: 1,
            timestamp_ms: 1_700_000_000_005,
        },
        ControlMessage::KeyboardLayoutInfo { layout_hint },
    ];

    // Encode all messages into a single byte stream (simulating IPC)
    let mut wire_data = Vec::new();
    for msg in &messages {
        let frame = msg.to_frame();
        wire_data.extend_from_slice(&frame.encode());
    }

    // Decode from the wire stream
    let (decoded_frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(decoded_frames.len(), messages.len());

    // Verify each message round-trips correctly
    for (frame, original_msg) in decoded_frames.iter().zip(messages.iter()) {
        assert_eq!(frame.channel, ChannelId::Control);
        let decoded_msg = ControlMessage::decode(&frame.payload).unwrap();
        assert_eq!(&decoded_msg, original_msg);

        // Also verify via Message dispatch
        let dispatched = Message::from_frame(frame).unwrap();
        assert!(matches!(dispatched, Message::Control(_)));
    }
}

#[test]
fn full_pipeline_input_messages() {
    let messages = vec![
        InputMessage::MouseMove { x: 960, y: 540 },
        InputMessage::MouseButton {
            button: MouseButton::Left,
            down: true,
            x: 960,
            y: 540,
        },
        InputMessage::MouseButton {
            button: MouseButton::Left,
            down: false,
            x: 960,
            y: 540,
        },
        InputMessage::KeyEvent {
            keycode: 30, // A
            down: true,
            modifiers: Modifiers::CTRL,
        },
        InputMessage::KeyEvent {
            keycode: 30,
            down: false,
            modifiers: Modifiers::empty(),
        },
        InputMessage::MouseScroll { dx: 0, dy: -3 },
        // KeyEventEx: 'a' on AZERTY layout (physical Q position → key_char 'a')
        InputMessage::KeyEventEx {
            keycode: 16, // physical KeyQ
            down: true,
            modifiers: Modifiers::empty(),
            key_char: 0x61, // 'a'
        },
        InputMessage::KeyEventEx {
            keycode: 16,
            down: false,
            modifiers: Modifiers::empty(),
            key_char: 0x61,
        },
        // KeyEventEx: non-printable key (Escape), key_char = 0
        InputMessage::KeyEventEx {
            keycode: 1,
            down: true,
            modifiers: Modifiers::empty(),
            key_char: 0,
        },
        // KeyEventEx: AltGr+E = € (U+20AC)
        InputMessage::KeyEventEx {
            keycode: 18,
            down: true,
            modifiers: Modifiers::ALT,
            key_char: 0x20AC,
        },
    ];

    let mut wire_data = Vec::new();
    for msg in &messages {
        wire_data.extend_from_slice(&msg.to_frame().encode());
    }

    let (frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(frames.len(), messages.len());

    for (frame, original) in frames.iter().zip(messages.iter()) {
        assert_eq!(frame.channel, ChannelId::Input);
        let decoded = InputMessage::decode(&frame.payload).unwrap();
        assert_eq!(&decoded, original);
    }
}

#[test]
fn full_pipeline_mixed_channels() {
    // Simulate a realistic sequence of messages from different channels
    let mut wire_data = Vec::new();

    // Session setup
    let ready = ControlMessage::SessionReady {
        version: 2,
        flags: SessionFlags::all(),
    };
    wire_data.extend_from_slice(&ready.to_frame().encode());

    // Client sends resize
    let resize = ControlMessage::ResolutionRequest {
        width: 1280,
        height: 720,
    };
    wire_data.extend_from_slice(&resize.to_frame().encode());

    // Server acks
    let ack = ControlMessage::ResolutionAck {
        width: 1280,
        height: 720,
    };
    wire_data.extend_from_slice(&ack.to_frame().encode());

    // Client sends keyboard layout hint
    let mut layout_hint = [0u8; 32];
    layout_hint[..2].copy_from_slice(b"de");
    let layout = ControlMessage::KeyboardLayoutInfo { layout_hint };
    wire_data.extend_from_slice(&layout.to_frame().encode());

    // Video frame (raw channel)
    let video = Frame::new(ChannelId::Video, vec![0x00, 0x00, 0x00, 0x01, 0x65, 0xAA]);
    wire_data.extend_from_slice(&video.encode());

    // Input events (legacy KeyEvent)
    let mouse = InputMessage::MouseMove { x: 100, y: 200 };
    wire_data.extend_from_slice(&mouse.to_frame().encode());

    // Input event (KeyEventEx — 'a' from AZERTY)
    let key_ex = InputMessage::KeyEventEx {
        keycode: 16,
        down: true,
        modifiers: Modifiers::empty(),
        key_char: 0x61,
    };
    wire_data.extend_from_slice(&key_ex.to_frame().encode());

    // Cursor update
    let cursor = CursorMessage::CursorMove { x: 100, y: 200 };
    wire_data.extend_from_slice(&cursor.to_frame().encode());

    // Clipboard
    let clip = ClipboardMessage::Text {
        content: b"Hello from remote".to_vec(),
    };
    wire_data.extend_from_slice(&clip.to_frame().encode());

    // Decode all
    let (frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(frames.len(), 9);

    // Verify channel routing
    let channels: Vec<ChannelId> = frames.iter().map(|f| f.channel).collect();
    assert_eq!(
        channels,
        vec![
            ChannelId::Control,
            ChannelId::Control,
            ChannelId::Control,
            ChannelId::Control,
            ChannelId::Video,
            ChannelId::Input,
            ChannelId::Input,
            ChannelId::Cursor,
            ChannelId::Clipboard,
        ]
    );

    // Verify message dispatch
    for frame in &frames {
        let msg = Message::from_frame(frame).unwrap();
        match frame.channel {
            ChannelId::Control => assert!(matches!(msg, Message::Control(_))),
            ChannelId::Video => assert!(matches!(msg, Message::Video(_))),
            ChannelId::Input => assert!(matches!(msg, Message::Input(_))),
            ChannelId::Cursor => assert!(matches!(msg, Message::Cursor(_))),
            ChannelId::Clipboard => assert!(matches!(msg, Message::Clipboard(_))),
            _ => panic!("unexpected channel"),
        }
    }
}

/// Test the video fragmentation and reassembly pipeline end-to-end.
#[test]
fn video_fragmentation_pipeline() {
    // Simulate a large H.264 IDR frame (>MTU)
    let mut nal_data = vec![0x00, 0x00, 0x00, 0x01, 0x65]; // Start code + IDR
    nal_data.extend(vec![0xBB; 5000]); // 5KB payload

    // Fragment into MTU-sized datagrams
    let max_fragment = 1100;
    let fragments = VideoDatagram::fragment(1, true, 33_333, &nal_data, max_fragment);
    assert!(fragments.len() > 1);
    assert_eq!(fragments[0].nal_id, 1);
    assert!(fragments[0].is_keyframe);

    // Encode each fragment, wrap in frames, write to wire
    let mut wire_data = Vec::new();
    for frag in &fragments {
        let payload = frag.encode();
        let frame = Frame::new(ChannelId::Video, payload);
        wire_data.extend_from_slice(&frame.encode());
    }

    // Read back from wire
    let (frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(frames.len(), fragments.len());

    // Decode fragments
    let decoded_frags: Vec<VideoDatagram> = frames
        .iter()
        .map(|f| VideoDatagram::decode(&f.payload).unwrap())
        .collect();

    // Reassemble
    let reassembled = VideoDatagram::reassemble(&decoded_frags).unwrap();
    assert_eq!(reassembled, nal_data);
}

/// Test file transfer message pipeline.
#[test]
fn file_transfer_pipeline() {
    let mut filename = [0u8; 256];
    let name = b"document.pdf";
    filename[..name.len()].copy_from_slice(name);

    let mut mime = [0u8; 64];
    let mt = b"application/pdf";
    mime[..mt.len()].copy_from_slice(mt);

    let file_data = vec![0xDE; 200_000]; // 200KB file
    let chunk_size = 65536; // 64KB chunks

    let mut wire_data = Vec::new();

    // File header
    let header = FileMessage::header(42, filename, file_data.len() as u64, mime);
    wire_data.extend_from_slice(&header.to_frame(ChannelId::FileDown).encode());

    // File chunks
    for (seq, chunk) in file_data.chunks(chunk_size).enumerate() {
        let msg = FileMessage::chunk(42, seq as u32, chunk.to_vec());
        wire_data.extend_from_slice(&msg.to_frame(ChannelId::FileDown).encode());
    }

    // File complete
    let complete = FileMessage::complete(42);
    wire_data.extend_from_slice(&complete.to_frame(ChannelId::FileDown).encode());

    // Decode all
    let (frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());

    // All frames on FileDown channel
    assert!(frames.iter().all(|f| f.channel == ChannelId::FileDown));

    // Verify file reassembly
    let mut received_data = Vec::new();
    let mut got_header = false;
    let mut got_complete = false;

    for frame in &frames {
        let msg = FileMessage::decode_on_channel(&frame.payload, frame.channel).unwrap();
        match msg {
            FileMessage::FileHeader {
                id,
                filename: _,
                size,
                ..
            } => {
                assert_eq!(id, 42);
                assert_eq!(size, 200_000);
                got_header = true;
            }
            FileMessage::FileChunk { id, data, .. } => {
                assert_eq!(id, 42);
                received_data.extend_from_slice(&data);
            }
            FileMessage::FileComplete { id } => {
                assert_eq!(id, 42);
                got_complete = true;
            }
        }
    }

    assert!(got_header);
    assert!(got_complete);
    assert_eq!(received_data, file_data);
}

/// Test incremental frame parsing (simulating chunked network reads).
#[test]
fn incremental_frame_parsing() {
    let ctrl_ping = ControlMessage::Ping {
        seq: 1,
        timestamp_ms: 100,
    };
    let ctrl_pong = ControlMessage::Pong {
        seq: 1,
        timestamp_ms: 105,
    };
    let mouse = InputMessage::MouseMove { x: 50, y: 50 };

    let mut wire_data = Vec::new();
    wire_data.extend_from_slice(&ctrl_ping.to_frame().encode());
    wire_data.extend_from_slice(&ctrl_pong.to_frame().encode());
    wire_data.extend_from_slice(&mouse.to_frame().encode());

    // Simulate reading in small chunks (3 bytes at a time)
    let mut buffer = Vec::new();
    let mut all_frames = Vec::new();
    let chunk_size = 3;

    for chunk in wire_data.chunks(chunk_size) {
        buffer.extend_from_slice(chunk);

        // Try to parse frames from buffer
        loop {
            if buffer.len() < FRAME_HEADER_SIZE {
                break;
            }
            match Frame::decode(&buffer) {
                Ok((frame, consumed)) => {
                    all_frames.push(frame);
                    buffer.drain(..consumed);
                }
                Err(FrameError::BufferTooShort { .. }) => break,
                Err(e) => panic!("unexpected error: {e}"),
            }
        }
    }

    assert_eq!(all_frames.len(), 3);
    assert!(buffer.is_empty());
}

/// Test audio frame pipeline.
#[test]
fn audio_frame_pipeline() {
    let frames: Vec<AudioFrame> = (0..10)
        .map(|i| AudioFrame {
            seq: i,
            timestamp_us: i as u64 * 20_000, // 20ms per frame
            data: vec![0x42; 160],           // 160 bytes of Opus data
        })
        .collect();

    let mut wire_data = Vec::new();
    for audio_frame in &frames {
        let payload = audio_frame.encode();
        let frame = Frame::new(ChannelId::AudioOut, payload);
        wire_data.extend_from_slice(&frame.encode());
    }

    let (decoded_frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(decoded_frames.len(), frames.len());

    for (wire_frame, original) in decoded_frames.iter().zip(frames.iter()) {
        assert_eq!(wire_frame.channel, ChannelId::AudioOut);
        let decoded = AudioFrame::decode(&wire_frame.payload).unwrap();
        assert_eq!(decoded.seq, original.seq);
        assert_eq!(decoded.timestamp_us, original.timestamp_us);
        assert_eq!(decoded.data, original.data);
    }
}

/// Test PCM audio pipeline: 20ms frames at 48kHz stereo s16le (3840 bytes each).
/// Uses to_frame_out() convenience method.
#[test]
fn audio_frame_pcm_pipeline() {
    // Simulate 1 second of audio = 50 frames of 20ms each
    let frame_count = 50;
    let pcm_frame_size = 3840; // 960 samples * 2 channels * 2 bytes

    let frames: Vec<AudioFrame> = (0..frame_count)
        .map(|i| {
            // Generate a recognizable pattern per frame
            let pattern = (i as u8).wrapping_mul(37);
            AudioFrame {
                seq: i,
                timestamp_us: i as u64 * 20_000,
                data: vec![pattern; pcm_frame_size],
            }
        })
        .collect();

    // Encode using to_frame_out() and serialize to wire
    let mut wire_data = Vec::new();
    for af in &frames {
        wire_data.extend_from_slice(&af.to_frame_out().encode());
    }

    // Decode from wire
    let (decoded_frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(decoded_frames.len(), frame_count as usize);

    // Verify each frame
    for (wire_frame, original) in decoded_frames.iter().zip(frames.iter()) {
        assert_eq!(wire_frame.channel, ChannelId::AudioOut);

        // Verify via Message dispatch
        let msg = Message::from_frame(wire_frame).unwrap();
        match msg {
            Message::AudioOut(payload) => {
                let decoded = AudioFrame::decode(&payload).unwrap();
                assert_eq!(decoded.seq, original.seq);
                assert_eq!(decoded.timestamp_us, original.timestamp_us);
                assert_eq!(decoded.data.len(), pcm_frame_size);
                assert_eq!(decoded.data, original.data);
            }
            other => panic!("expected AudioOut, got {:?}", other),
        }
    }
}

/// Test audio frames interleaved with video and control in a realistic session.
#[test]
fn audio_interleaved_with_video_and_control() {
    let mut wire_data = Vec::new();

    // Session ready (with AUDIO flag)
    let ready = ControlMessage::SessionReady {
        version: 2,
        flags: SessionFlags::AUDIO | SessionFlags::CLIPBOARD,
    };
    wire_data.extend_from_slice(&ready.to_frame().encode());

    // Resolution ack
    let ack = ControlMessage::ResolutionAck {
        width: 1280,
        height: 720,
    };
    wire_data.extend_from_slice(&ack.to_frame().encode());

    // Video frame
    let video = Frame::new(ChannelId::Video, vec![0x00, 0x00, 0x00, 0x01, 0x65]);
    wire_data.extend_from_slice(&video.encode());

    // Audio frames (3 x 20ms)
    for i in 0..3u32 {
        let af = AudioFrame {
            seq: i,
            timestamp_us: i as u64 * 20_000,
            data: vec![0x80; 3840],
        };
        wire_data.extend_from_slice(&af.to_frame_out().encode());
    }

    // Another video frame
    let video2 = Frame::new(ChannelId::Video, vec![0x00, 0x00, 0x01, 0x41, 0xBB]);
    wire_data.extend_from_slice(&video2.encode());

    // Cursor update
    let cursor = CursorMessage::CursorMove { x: 640, y: 360 };
    wire_data.extend_from_slice(&cursor.to_frame().encode());

    // More audio
    let af = AudioFrame {
        seq: 3,
        timestamp_us: 60_000,
        data: vec![0x81; 3840],
    };
    wire_data.extend_from_slice(&af.to_frame_out().encode());

    // Decode all
    let (frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(frames.len(), 9); // 2 control + 2 video + 4 audio + 1 cursor

    // Verify channel sequence
    let channels: Vec<ChannelId> = frames.iter().map(|f| f.channel).collect();
    assert_eq!(
        channels,
        vec![
            ChannelId::Control,
            ChannelId::Control,
            ChannelId::Video,
            ChannelId::AudioOut,
            ChannelId::AudioOut,
            ChannelId::AudioOut,
            ChannelId::Video,
            ChannelId::Cursor,
            ChannelId::AudioOut,
        ]
    );

    // Verify all audio frames decode correctly
    let audio_frames: Vec<AudioFrame> = frames
        .iter()
        .filter(|f| f.channel == ChannelId::AudioOut)
        .map(|f| AudioFrame::decode(&f.payload).unwrap())
        .collect();
    assert_eq!(audio_frames.len(), 4);
    assert_eq!(audio_frames[0].seq, 0);
    assert_eq!(audio_frames[1].seq, 1);
    assert_eq!(audio_frames[2].seq, 2);
    assert_eq!(audio_frames[3].seq, 3);
    assert_eq!(audio_frames[3].data.len(), 3840);
}

/// Test incremental parsing of audio frames (chunked network reads).
#[test]
fn audio_frame_incremental_parsing() {
    let mut wire_data = Vec::new();
    for i in 0..5u32 {
        let af = AudioFrame {
            seq: i,
            timestamp_us: i as u64 * 20_000,
            data: vec![(i & 0xFF) as u8; 3840],
        };
        wire_data.extend_from_slice(&af.to_frame_out().encode());
    }

    // Simulate reading in 500-byte chunks (less than a full frame)
    let mut buffer = Vec::new();
    let mut all_frames = Vec::new();
    let chunk_size = 500;

    for chunk in wire_data.chunks(chunk_size) {
        buffer.extend_from_slice(chunk);

        loop {
            if buffer.len() < FRAME_HEADER_SIZE {
                break;
            }
            match Frame::decode(&buffer) {
                Ok((frame, consumed)) => {
                    all_frames.push(frame);
                    buffer.drain(..consumed);
                }
                Err(FrameError::BufferTooShort { .. }) => break,
                Err(e) => panic!("unexpected error: {e}"),
            }
        }
    }

    assert_eq!(all_frames.len(), 5);
    assert!(buffer.is_empty());

    for (i, frame) in all_frames.iter().enumerate() {
        assert_eq!(frame.channel, ChannelId::AudioOut);
        let af = AudioFrame::decode(&frame.payload).unwrap();
        assert_eq!(af.seq, i as u32);
        assert_eq!(af.data.len(), 3840);
    }
}

/// Test tile messages in a full pipeline.
#[test]
fn tile_pipeline_all_variants() {
    let mut wire_data = Vec::new();

    // GridConfig
    let gc = TileMessage::GridConfig {
        tile_size: 64,
        cols: 20,
        rows: 12,
        screen_w: 1280,
        screen_h: 768,
    };
    wire_data.extend_from_slice(&gc.to_frame().encode());

    // Fill
    let fill = TileMessage::Fill {
        col: 0,
        row: 0,
        rgba: 0xFF_FF_FF_FF,
    };
    wire_data.extend_from_slice(&fill.to_frame().encode());

    // CacheHit
    let hit = TileMessage::CacheHit {
        col: 1,
        row: 1,
        hash: 0xDEADBEEF_CAFEBABE,
    };
    wire_data.extend_from_slice(&hit.to_frame().encode());

    // Qoi
    let qoi = TileMessage::Qoi {
        col: 2,
        row: 3,
        hash: 0x1234_5678_9ABC_DEF0,
        data: vec![0x71; 100], // fake QOI data
    };
    wire_data.extend_from_slice(&qoi.to_frame().encode());

    // Zstd
    let zstd = TileMessage::Zstd {
        col: 5,
        row: 6,
        hash: 0xFEDC_BA98_7654_3210,
        data: vec![0x28; 80], // fake zstd data
    };
    wire_data.extend_from_slice(&zstd.to_frame().encode());

    // VideoRegion
    let vr = TileMessage::VideoRegion {
        x: 100,
        y: 200,
        w: 800,
        h: 600,
    };
    wire_data.extend_from_slice(&vr.to_frame().encode());

    // ScrollCopy with negative offsets
    let scroll = TileMessage::ScrollCopy {
        dx: -5,
        dy: 10,
        region_top: 64,
        region_bottom: 704,
        region_right: 1280,
    };
    wire_data.extend_from_slice(&scroll.to_frame().encode());

    // GridOffset
    let offset = TileMessage::GridOffset {
        offset_x: -32,
        offset_y: 16,
    };
    wire_data.extend_from_slice(&offset.to_frame().encode());

    // TileDrawMode
    let mode = TileMessage::TileDrawMode {
        apply_offset: false,
    };
    wire_data.extend_from_slice(&mode.to_frame().encode());

    // ScrollStats
    let stats = TileMessage::ScrollStats {
        scroll_batches_total: 100,
        scroll_full_fallbacks_total: 5,
        scroll_potential_tiles_total: 2000,
        scroll_saved_tiles_total: 1800,
        scroll_non_quantized_fallbacks_total: 2,
        scroll_residual_full_repaints_total: 3,
        scroll_zero_saved_batches_total: 4,
    };
    wire_data.extend_from_slice(&stats.to_frame().encode());

    // CacheMiss
    let miss = TileMessage::CacheMiss {
        frame_seq: 42,
        col: 3,
        row: 4,
        hash: 0xABCD_EF01_2345_6789,
    };
    wire_data.extend_from_slice(&miss.to_frame().encode());

    // BatchEnd
    let batch = TileMessage::BatchEnd { frame_seq: 9999 };
    wire_data.extend_from_slice(&batch.to_frame().encode());

    // Decode all
    let (frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(frames.len(), 12);

    // All frames on Tiles channel
    assert!(frames.iter().all(|f| f.channel == ChannelId::Tiles));

    // Verify each via Message dispatch
    for frame in &frames {
        let msg = Message::from_frame(frame).unwrap();
        assert!(matches!(msg, Message::Tiles(_)));
    }

    // Verify individual messages decode correctly
    assert_eq!(TileMessage::decode(&frames[0].payload).unwrap(), gc);
    assert_eq!(TileMessage::decode(&frames[1].payload).unwrap(), fill);
    assert_eq!(TileMessage::decode(&frames[2].payload).unwrap(), hit);
    assert_eq!(TileMessage::decode(&frames[3].payload).unwrap(), qoi);
    assert_eq!(TileMessage::decode(&frames[4].payload).unwrap(), zstd);
    assert_eq!(TileMessage::decode(&frames[5].payload).unwrap(), vr);
    assert_eq!(TileMessage::decode(&frames[6].payload).unwrap(), scroll);
    assert_eq!(TileMessage::decode(&frames[7].payload).unwrap(), offset);
    assert_eq!(TileMessage::decode(&frames[8].payload).unwrap(), mode);
    assert_eq!(TileMessage::decode(&frames[9].payload).unwrap(), stats);
    assert_eq!(TileMessage::decode(&frames[10].payload).unwrap(), miss);
    assert_eq!(TileMessage::decode(&frames[11].payload).unwrap(), batch);
}

/// Test ResolutionLocked in a mixed channel scenario.
#[test]
fn resolution_locked_in_multi_client_session() {
    let mut wire_data = Vec::new();

    // Server sends SessionReady
    let ready = ControlMessage::SessionReady {
        version: 2,
        flags: SessionFlags::all(),
    };
    wire_data.extend_from_slice(&ready.to_frame().encode());

    // Owner sets resolution
    let ack = ControlMessage::ResolutionAck {
        width: 1920,
        height: 1080,
    };
    wire_data.extend_from_slice(&ack.to_frame().encode());

    // Non-owner gets ResolutionLocked
    let locked = ControlMessage::ResolutionLocked {
        width: 1920,
        height: 1080,
    };
    wire_data.extend_from_slice(&locked.to_frame().encode());

    // BitrateHint for adaptation
    let hint = ControlMessage::BitrateHint {
        target_bps: 2_000_000,
    };
    wire_data.extend_from_slice(&hint.to_frame().encode());

    let (frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(frames.len(), 4);

    // Verify ResolutionLocked round-trip
    let msg = ControlMessage::decode(&frames[2].payload).unwrap();
    assert!(matches!(
        msg,
        ControlMessage::ResolutionLocked {
            width: 1920,
            height: 1080
        }
    ));
}

/// Test concurrent fragment reassembly of two interleaved NAL units.
#[test]
fn interleaved_fragment_reassembly() {
    // Two different NAL units being fragmented and interleaved
    let nal_1 = vec![0xAA; 3000]; // IDR frame
    let nal_2 = vec![0xBB; 2000]; // P frame

    let frags_1 = VideoDatagram::fragment(1, true, 33_333, &nal_1, 1000);
    let frags_2 = VideoDatagram::fragment(2, false, 66_666, &nal_2, 800);

    // Interleave fragments (simulating out-of-order network delivery)
    let mut all_frags = Vec::new();
    let mut i1 = frags_1.iter().peekable();
    let mut i2 = frags_2.iter().peekable();
    let mut toggle = true;
    loop {
        if toggle {
            if let Some(f) = i1.next() {
                all_frags.push(f.clone());
            }
        } else if let Some(f) = i2.next() {
            all_frags.push(f.clone());
        }
        toggle = !toggle;
        if i1.peek().is_none() && i2.peek().is_none() {
            break;
        }
    }

    // Separate by nal_id and reassemble each
    let group_1: Vec<_> = all_frags
        .iter()
        .filter(|f| f.nal_id == 1)
        .cloned()
        .collect();
    let group_2: Vec<_> = all_frags
        .iter()
        .filter(|f| f.nal_id == 2)
        .cloned()
        .collect();

    let reassembled_1 = VideoDatagram::reassemble(&group_1).unwrap();
    let reassembled_2 = VideoDatagram::reassemble(&group_2).unwrap();

    assert_eq!(reassembled_1, nal_1);
    assert_eq!(reassembled_2, nal_2);
    assert!(group_1[0].is_keyframe);
    assert!(!group_2[0].is_keyframe);
}

/// Stress test: encode and decode many messages of mixed types.
#[test]
fn stress_test_mixed_messages() {
    let mut wire_data = Vec::new();
    let message_count = 1000;

    for i in 0..message_count {
        match i % 6 {
            0 => {
                let msg = ControlMessage::Ping {
                    seq: i,
                    timestamp_ms: i as u64 * 1000,
                };
                wire_data.extend_from_slice(&msg.to_frame().encode());
            }
            1 => {
                let msg = InputMessage::MouseMove {
                    x: (i % 1920) as u16,
                    y: (i % 1080) as u16,
                };
                wire_data.extend_from_slice(&msg.to_frame().encode());
            }
            2 => {
                let msg = CursorMessage::CursorMove {
                    x: (i % 1920) as u16,
                    y: (i % 1080) as u16,
                };
                wire_data.extend_from_slice(&msg.to_frame().encode());
            }
            3 => {
                let af = AudioFrame {
                    seq: i,
                    timestamp_us: i as u64 * 20_000,
                    data: vec![0x42; 160],
                };
                wire_data.extend_from_slice(&af.to_frame_out().encode());
            }
            4 => {
                let msg = TileMessage::Fill {
                    col: (i % 20) as u16,
                    row: (i % 12) as u16,
                    rgba: i,
                };
                wire_data.extend_from_slice(&msg.to_frame().encode());
            }
            5 => {
                let msg = TileMessage::BatchEnd { frame_seq: i };
                wire_data.extend_from_slice(&msg.to_frame().encode());
            }
            _ => unreachable!(),
        }
    }

    let (frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert_eq!(frames.len(), message_count as usize);

    // Verify all frames can be dispatched
    for frame in &frames {
        let msg = Message::from_frame(frame).unwrap();
        match frame.channel {
            ChannelId::Control => assert!(matches!(msg, Message::Control(_))),
            ChannelId::Input => assert!(matches!(msg, Message::Input(_))),
            ChannelId::Cursor => assert!(matches!(msg, Message::Cursor(_))),
            ChannelId::AudioOut => assert!(matches!(msg, Message::AudioOut(_))),
            ChannelId::Tiles => assert!(matches!(msg, Message::Tiles(_))),
            _ => panic!("unexpected channel {:?}", frame.channel),
        }
    }
}

/// Test video fragmentation with tile info preserved across fragments.
#[test]
fn video_fragmentation_with_tile_info() {
    let tile_info = VideoTileInfo {
        tile_x: 128,
        tile_y: 256,
        tile_w: 640,
        tile_h: 480,
        screen_w: 1920,
        screen_h: 1080,
    };

    let nal_data = vec![0xCC; 4000];
    let mut frags = VideoDatagram::fragment(5, true, 100_000, &nal_data, 1200);

    // Attach tile info to all fragments (as the protocol requires)
    for frag in &mut frags {
        frag.tile_info = Some(tile_info);
    }

    // Encode, wrap in frames, decode
    let mut wire_data = Vec::new();
    for frag in &frags {
        let payload = frag.encode();
        wire_data.extend_from_slice(&Frame::new(ChannelId::Video, payload).encode());
    }

    let (frames, _) = Frame::decode_all(&wire_data).unwrap();
    let decoded_frags: Vec<VideoDatagram> = frames
        .iter()
        .map(|f| VideoDatagram::decode(&f.payload).unwrap())
        .collect();

    // All fragments should have tile info
    for frag in &decoded_frags {
        assert_eq!(frag.tile_info, Some(tile_info));
    }

    // Reassemble should still work
    let reassembled = VideoDatagram::reassemble(&decoded_frags).unwrap();
    assert_eq!(reassembled, nal_data);
}

/// Test file upload pipeline (C->S direction).
#[test]
fn file_upload_pipeline() {
    let mut filename = [0u8; 256];
    filename[..9].copy_from_slice(b"photo.png");

    let mut mime = [0u8; 64];
    mime[..9].copy_from_slice(b"image/png");

    let file_data = vec![0x89; 50_000]; // 50KB file
    let chunk_size = 16384; // 16KB chunks

    let mut wire_data = Vec::new();

    let header = FileMessage::header(1, filename, file_data.len() as u64, mime);
    wire_data.extend_from_slice(&header.to_frame(ChannelId::FileUp).encode());

    for (seq, chunk) in file_data.chunks(chunk_size).enumerate() {
        let msg = FileMessage::chunk(1, seq as u32, chunk.to_vec());
        wire_data.extend_from_slice(&msg.to_frame(ChannelId::FileUp).encode());
    }

    let complete = FileMessage::complete(1);
    wire_data.extend_from_slice(&complete.to_frame(ChannelId::FileUp).encode());

    let (frames, consumed) = Frame::decode_all(&wire_data).unwrap();
    assert_eq!(consumed, wire_data.len());
    assert!(frames.iter().all(|f| f.channel == ChannelId::FileUp));

    // Reassemble and verify
    let mut received = Vec::new();
    for frame in &frames {
        let msg = FileMessage::decode_on_channel(&frame.payload, frame.channel).unwrap();
        if let FileMessage::FileChunk { data, .. } = msg {
            received.extend_from_slice(&data);
        }
    }
    assert_eq!(received, file_data);
}

/// Test that a full resize handshake encodes/decodes correctly.
#[test]
fn resize_handshake() {
    // Client sends resize request
    let request = ControlMessage::ResolutionRequest {
        width: 500,
        height: 500,
    };
    let req_frame = request.to_frame();
    let req_wire = req_frame.encode();

    // Server receives and processes
    let (decoded_frame, _) = Frame::decode(&req_wire).unwrap();
    let decoded_msg = ControlMessage::decode(&decoded_frame.payload).unwrap();
    assert!(matches!(
        decoded_msg,
        ControlMessage::ResolutionRequest {
            width: 500,
            height: 500
        }
    ));

    // Server sends ack
    let ack = ControlMessage::ResolutionAck {
        width: 500,
        height: 500,
    };
    let ack_wire = ack.to_frame().encode();

    // Client receives ack
    let (ack_frame, _) = Frame::decode(&ack_wire).unwrap();
    let ack_msg = ControlMessage::decode(&ack_frame.payload).unwrap();
    assert!(matches!(
        ack_msg,
        ControlMessage::ResolutionAck {
            width: 500,
            height: 500
        }
    ));
}
