use std::{collections::BTreeSet, sync::OnceLock};

use bpane_protocol::{
    channel::ChannelId,
    frame::{Frame, FrameError, Message},
    AudioFrame, ClipboardMessage, ControlMessage, CursorMessage, FileMessage, InputMessage,
    Modifiers, SessionFlags, TileMessage, VideoDatagram, VideoTileInfo,
};
use serde::Deserialize;

const EXPECTED_FIXTURE_COUNT: usize = 15;
const KNOWN_CAPABILITIES: &[&str] = &[
    "audio_adpcm_ima_stereo",
    "audio_opus",
    "audio_pcm_s16le",
    "camera_h264_annex_b",
    "client_access_state",
    "clipboard_text",
    "extended_keyboard",
    "file_transfer",
    "h264_video",
    "microphone_opus",
    "roi_video",
    "tile_cache",
    "tile_scroll",
    "tile_zstd",
];

#[derive(Debug, Deserialize)]
struct FixtureCatalog {
    schema_version: u8,
    catalog: String,
    vectors: Vec<WireFixture>,
}

#[derive(Debug, Deserialize)]
struct WireFixture {
    name: String,
    direction: String,
    transport: String,
    channel: String,
    capabilities: Vec<String>,
    wire_hex: String,
    expected: FixtureExpectation,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
enum FixtureExpectation {
    Valid { message: String },
    Invalid { error: String },
}

fn catalog() -> &'static FixtureCatalog {
    static CATALOG: OnceLock<FixtureCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        serde_json::from_str(include_str!("fixtures/wire-fixtures.json")).expect("fixture json")
    })
}

fn fixture(name: &str) -> &'static WireFixture {
    catalog()
        .vectors
        .iter()
        .find(|fixture| fixture.name == name)
        .expect("fixture exists")
}

fn wire(name: &str) -> Vec<u8> {
    hex_to_bytes(&fixture(name).wire_hex)
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
fn shared_catalog_schema_and_every_vector_are_consumed() {
    let catalog = catalog();
    assert_eq!(catalog.schema_version, 2);
    assert_eq!(catalog.catalog, "browserpane-v1-conformance");
    assert_eq!(catalog.vectors.len(), EXPECTED_FIXTURE_COUNT);

    let mut names = BTreeSet::new();
    for fixture in &catalog.vectors {
        assert!(
            names.insert(fixture.name.as_str()),
            "duplicate fixture name"
        );
        validate_metadata(fixture);

        let actual = classify_fixture(fixture);
        match &fixture.expected {
            FixtureExpectation::Valid { message } => {
                assert_eq!(actual, Ok(message.as_str()), "fixture {}", fixture.name);
            }
            FixtureExpectation::Invalid { error } => {
                assert_eq!(actual, Err(error.as_str()), "fixture {}", fixture.name);
            }
        }
    }
}

fn validate_metadata(fixture: &WireFixture) {
    assert!(
        matches!(
            fixture.direction.as_str(),
            "client_to_server" | "server_to_client" | "bidirectional"
        ),
        "invalid direction for {}",
        fixture.name
    );
    assert!(
        matches!(
            fixture.transport.as_str(),
            "reliable_frame" | "datagram_payload"
        ),
        "invalid transport for {}",
        fixture.name
    );
    assert!(
        matches!(
            fixture.channel.as_str(),
            "video"
                | "audio_out"
                | "audio_in"
                | "video_in"
                | "input"
                | "cursor"
                | "clipboard"
                | "file_up"
                | "file_down"
                | "control"
                | "tiles"
        ),
        "invalid channel for {}",
        fixture.name
    );
    assert!(
        fixture
            .capabilities
            .windows(2)
            .all(|pair| pair[0] < pair[1]),
        "capabilities must be sorted and unique for {}",
        fixture.name
    );
    assert!(
        fixture
            .capabilities
            .iter()
            .all(|capability| KNOWN_CAPABILITIES.contains(&capability.as_str())),
        "unknown capability for {}",
        fixture.name
    );
    assert!(fixture.wire_hex.len().is_multiple_of(2));
    assert!(fixture
        .wire_hex
        .bytes()
        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
}

fn classify_fixture(fixture: &WireFixture) -> Result<&'static str, &'static str> {
    let bytes = hex_to_bytes(&fixture.wire_hex);
    if fixture.transport == "datagram_payload" {
        return VideoDatagram::decode(&bytes)
            .map(|_| "video_datagram")
            .map_err(frame_error_class);
    }

    if channel_name(bytes.first().copied()) != Some(fixture.channel.as_str()) {
        return Err("channel_metadata_mismatch");
    }

    let (frame, consumed) = Frame::decode(&bytes).map_err(frame_error_class)?;
    if consumed != bytes.len() {
        return Err("trailing_data");
    }

    match frame.channel {
        ChannelId::AudioOut | ChannelId::AudioIn => AudioFrame::decode(&frame.payload)
            .map(|_| "audio_frame")
            .map_err(frame_error_class),
        ChannelId::Video => VideoDatagram::decode(&frame.payload)
            .map(|_| "video_datagram")
            .map_err(frame_error_class),
        _ => Message::from_frame(&frame)
            .map(message_class)
            .map_err(frame_error_class),
    }
}

fn message_class(message: Message) -> &'static str {
    match message {
        Message::Control(ControlMessage::SessionReady { .. }) => "control_session_ready",
        Message::Input(InputMessage::KeyEventEx { .. }) => "input_key_event_ex",
        Message::Cursor(CursorMessage::CursorShape { .. }) => "cursor_shape",
        Message::Clipboard(ClipboardMessage::Text { .. }) => "clipboard_text",
        Message::FileUp(FileMessage::FileHeader { .. }) => "file_header",
        Message::FileDown(FileMessage::FileChunk { .. }) => "file_chunk",
        Message::FileDown(FileMessage::FileComplete { .. }) => "file_complete",
        Message::Tiles(TileMessage::GridConfig { .. }) => "tile_grid_config",
        Message::Tiles(TileMessage::ScrollStats { .. }) => "tile_scroll_stats",
        Message::Tiles(TileMessage::Zstd { .. }) => "tile_zstd",
        _ => "unsupported_catalog_message",
    }
}

fn channel_name(channel: Option<u8>) -> Option<&'static str> {
    match channel.and_then(ChannelId::from_u8) {
        Some(ChannelId::Video) => Some("video"),
        Some(ChannelId::AudioOut) => Some("audio_out"),
        Some(ChannelId::AudioIn) => Some("audio_in"),
        Some(ChannelId::VideoIn) => Some("video_in"),
        Some(ChannelId::Input) => Some("input"),
        Some(ChannelId::Cursor) => Some("cursor"),
        Some(ChannelId::Clipboard) => Some("clipboard"),
        Some(ChannelId::FileUp) => Some("file_up"),
        Some(ChannelId::FileDown) => Some("file_down"),
        Some(ChannelId::Control) => Some("control"),
        Some(ChannelId::Tiles) => Some("tiles"),
        None => None,
    }
}

fn frame_error_class(error: FrameError) -> &'static str {
    match error {
        FrameError::BufferTooShort { .. } => "buffer_too_short",
        FrameError::UnknownChannel(_) => "unknown_channel",
        FrameError::UnknownMessageType { .. } => "unknown_message_type",
        FrameError::InvalidFieldValue { .. } => "invalid_field_value",
        FrameError::PayloadTooLarge(_) => "payload_too_large",
        FrameError::TrailingData(_) => "trailing_data",
    }
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
            scroll_non_quantized_fallbacks_total: 1,
            scroll_residual_full_repaints_total: 1,
            scroll_residual_interior_limit_fallbacks_total: 1,
            scroll_residual_low_saved_ratio_fallbacks_total: 0,
            scroll_residual_large_row_shift_fallbacks_total: 0,
            scroll_residual_other_fallbacks_total: 0,
            scroll_zero_saved_batches_total: 3,
            scroll_split_region_batches_total: 7,
            scroll_sticky_band_batches_total: 5,
            scroll_chrome_tiles_total: 640,
            scroll_exposed_strip_tiles_total: 128,
            scroll_interior_residual_tiles_total: 96,
            scroll_edge_strip_residual_tiles_total: 0,
            scroll_small_edge_strip_residual_tiles_total: 0,
            scroll_small_edge_strip_residual_rows_total: 0,
            scroll_small_edge_strip_residual_area_px_total: 0,
            host_sent_hash_entries: 0,
            host_sent_hash_evictions_total: 0,
            host_cache_miss_reports_total: 0,
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
