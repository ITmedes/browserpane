use std::{collections::BTreeMap, sync::OnceLock};

use bpane_protocol::{
    channel::ChannelId,
    frame::{Frame, FrameError, Message},
    AudioFrame, ClipboardMessage, ControlMessage, CursorMessage, FileMessage, InputMessage,
    Modifiers, SessionFlags, TileMessage, VideoDatagram, VideoTileInfo,
};

fn fixtures() -> &'static BTreeMap<String, String> {
    static FIXTURES: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        serde_json::from_str(include_str!("fixtures/wire-fixtures.json")).expect("fixture json")
    })
}

fn wire(name: &str) -> Vec<u8> {
    hex_to_bytes(fixtures().get(name).expect("fixture exists"))
}

fn hex_to_bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0, "hex fixture must have even length");
    hex.as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            u8::from_str_radix(std::str::from_utf8(pair).expect("utf8 hex"), 16).expect("hex")
        })
        .collect()
}

fn fixed<const N: usize>(text: &str) -> [u8; N] {
    let bytes = text.as_bytes();
    assert!(bytes.len() <= N, "fixture text must fit fixed field");
    let mut out = [0u8; N];
    out[..bytes.len()].copy_from_slice(bytes);
    out
}

#[test]
fn valid_wire_fixtures_match_exact_rust_encoders() {
    assert_eq!(
        ControlMessage::SessionReady {
            version: 1,
            flags: SessionFlags::new(0x35),
        }
        .to_frame()
        .encode()
        .to_vec(),
        wire("control_session_ready")
    );
    assert_eq!(
        InputMessage::KeyEventEx {
            keycode: 30,
            down: true,
            modifiers: Modifiers::empty(),
            key_char: u32::from(b'a'),
        }
        .to_frame()
        .encode()
        .to_vec(),
        wire("input_key_event_ex")
    );
    assert_eq!(
        CursorMessage::CursorShape {
            width: 16,
            height: 24,
            hotspot_x: 3,
            hotspot_y: 5,
            data: vec![0xde, 0xad, 0xbe, 0xef],
        }
        .to_frame()
        .encode()
        .to_vec(),
        wire("cursor_shape_small")
    );
    assert_eq!(
        ClipboardMessage::Text {
            content: b"hello clipboard".to_vec(),
        }
        .to_frame()
        .encode()
        .to_vec(),
        wire("clipboard_text")
    );
    assert_eq!(
        FileMessage::header(
            42,
            fixed("invoice.pdf"),
            123_456_789,
            fixed("application/pdf")
        )
        .to_frame(ChannelId::FileUp)
        .encode()
        .to_vec(),
        wire("file_header_upload")
    );
    assert_eq!(
        FileMessage::chunk(42, 3, vec![0x00, 0xff, 0x10, 0x20])
            .to_frame(ChannelId::FileDown)
            .encode()
            .to_vec(),
        wire("file_chunk_download")
    );
    assert_eq!(
        FileMessage::complete(42)
            .to_frame(ChannelId::FileDown)
            .encode()
            .to_vec(),
        wire("file_complete_download")
    );
    assert_eq!(
        TileMessage::GridConfig {
            tile_size: 256,
            cols: 12,
            rows: 8,
            screen_w: 1920,
            screen_h: 1080,
        }
        .to_frame()
        .encode()
        .to_vec(),
        wire("tile_grid_config")
    );
    assert_eq!(
        TileMessage::ScrollStats {
            scroll_batches_total: 11,
            scroll_full_fallbacks_total: 2,
            scroll_potential_tiles_total: 1_000,
            scroll_saved_tiles_total: 730,
        }
        .to_frame()
        .encode()
        .to_vec(),
        wire("tile_scroll_stats")
    );
    assert_eq!(
        TileMessage::Zstd {
            col: 2,
            row: 5,
            hash: 0x1122_3344_5566_7788,
            data: vec![1, 2, 3, 4, 5],
        }
        .to_frame()
        .encode()
        .to_vec(),
        wire("tile_zstd")
    );
    assert_eq!(
        AudioFrame {
            seq: 7,
            timestamp_us: 123_456,
            data: vec![0x57, 0x52, 0x41, 0x31, 0x02, 0x01, 0x02],
        }
        .to_frame_out()
        .encode()
        .to_vec(),
        wire("audio_out_frame")
    );
    assert_eq!(
        VideoDatagram {
            nal_id: 99,
            fragment_seq: 0,
            fragment_total: 1,
            is_keyframe: true,
            pts_us: 5_000,
            data: vec![0x00, 0x00, 0x01, 0x65, 0xaa, 0xbb],
            tile_info: Some(VideoTileInfo {
                tile_x: 100,
                tile_y: 200,
                tile_w: 320,
                tile_h: 180,
                screen_w: 1920,
                screen_h: 1080,
            }),
        }
        .encode(),
        wire("video_single_fragment_tile")
    );
}

#[test]
fn valid_wire_fixtures_decode_to_expected_messages() {
    let (control, _) = Frame::decode(&wire("control_session_ready")).expect("control frame");
    assert_eq!(control.channel, ChannelId::Control);
    assert_eq!(
        Message::from_frame(&control).expect("typed control"),
        Message::Control(ControlMessage::SessionReady {
            version: 1,
            flags: SessionFlags::new(0x35),
        })
    );

    let (input, _) = Frame::decode(&wire("input_key_event_ex")).expect("input frame");
    assert_eq!(
        Message::from_frame(&input).expect("typed input"),
        Message::Input(InputMessage::KeyEventEx {
            keycode: 30,
            down: true,
            modifiers: Modifiers::empty(),
            key_char: u32::from(b'a'),
        })
    );

    let (audio, _) = Frame::decode(&wire("audio_out_frame")).expect("audio frame");
    assert_eq!(
        AudioFrame::decode(&audio.payload)
            .expect("audio payload")
            .seq,
        7
    );

    let video = VideoDatagram::decode(&wire("video_single_fragment_tile")).expect("video datagram");
    assert_eq!(video.nal_id, 99);
    assert_eq!(video.tile_info.expect("tile info").screen_w, 1920);
}

#[test]
fn invalid_wire_fixtures_reject_as_expected() {
    assert_eq!(
        Frame::decode(&wire("invalid_frame_oversized_length")).expect_err("oversized length"),
        FrameError::PayloadTooLarge(2_147_483_649)
    );

    let (tile, _) = Frame::decode(&wire("invalid_tile_unknown_tag")).expect("tile frame");
    assert_eq!(
        TileMessage::decode(&tile.payload).expect_err("unknown tile tag"),
        FrameError::UnknownMessageType {
            channel: ChannelId::Tiles.as_u8(),
            tag: 0xff,
        }
    );

    let (file, _) = Frame::decode(&wire("invalid_file_chunk_truncated")).expect("file frame");
    assert_eq!(
        FileMessage::decode_on_channel(&file.payload, file.channel)
            .expect_err("truncated file chunk"),
        FrameError::BufferTooShort {
            expected: 17,
            available: 15,
        }
    );
}
