use alloc::vec;

use crate::TileMessage;

use super::super::FrameError;

#[test]
fn tile_grid_config_round_trip() {
    let msg = TileMessage::GridConfig {
        tile_size: 64,
        cols: 20,
        rows: 12,
        screen_w: 1280,
        screen_h: 768,
    };
    assert_eq!(msg, TileMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn tile_cache_variants_round_trip() {
    let hit = TileMessage::CacheHit {
        col: 5,
        row: 3,
        hash: 0xDEADBEEFCAFE1234,
    };
    let miss = TileMessage::CacheMiss {
        frame_seq: 77,
        col: 5,
        row: 3,
        hash: 0xDEADBEEFCAFE1234,
    };
    assert_eq!(hit, TileMessage::decode(&hit.encode()).unwrap());
    assert_eq!(miss, TileMessage::decode(&miss.encode()).unwrap());
}

#[test]
fn tile_fill_round_trip() {
    let msg = TileMessage::Fill {
        col: 0,
        row: 0,
        rgba: 0xFF3A3A6E,
    };
    assert_eq!(msg, TileMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn tile_qoi_and_zstd_round_trip() {
    let qoi = TileMessage::Qoi {
        col: 10,
        row: 7,
        hash: 0x1234567890ABCDEF,
        data: vec![0x71, 0x6f, 0x69, 0x66, 0x00, 0x00, 0x00, 0x40],
    };
    let zstd = TileMessage::Zstd {
        col: 3,
        row: 5,
        hash: 0xFEDCBA9876543210,
        data: vec![0x28, 0xB5, 0x2F, 0xFD, 0x00, 0x00, 0x01, 0x00],
    };
    assert_eq!(qoi, TileMessage::decode(&qoi.encode()).unwrap());
    assert_eq!(zstd, TileMessage::decode(&zstd.encode()).unwrap());
}

#[test]
fn tile_qoi_large_payload_round_trip() {
    let msg = TileMessage::Qoi {
        col: 1,
        row: 2,
        hash: 42,
        data: vec![0xAA; 65_536],
    };
    assert_eq!(msg, TileMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn tile_video_region_round_trip() {
    let msg = TileMessage::VideoRegion {
        x: 100,
        y: 200,
        w: 640,
        h: 480,
    };
    assert_eq!(msg, TileMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn tile_batch_end_round_trip() {
    let msg = TileMessage::BatchEnd { frame_seq: 12_345 };
    assert_eq!(msg, TileMessage::decode(&msg.encode()).unwrap());
}

#[test]
fn tile_scroll_messages_round_trip() {
    let copy = TileMessage::ScrollCopy {
        dx: 0,
        dy: -128,
        region_top: 80,
        region_bottom: 720,
        region_right: 1260,
    };
    let draw_mode = TileMessage::TileDrawMode { apply_offset: true };
    let stats = TileMessage::ScrollStats {
        scroll_batches_total: 42,
        scroll_full_fallbacks_total: 9,
        scroll_potential_tiles_total: 12_345,
        scroll_saved_tiles_total: 8_765,
        scroll_non_quantized_fallbacks_total: 4,
        scroll_residual_full_repaints_total: 5,
        scroll_zero_saved_batches_total: 6,
    };
    let offset = TileMessage::GridOffset {
        offset_x: -32,
        offset_y: 64,
    };

    assert_eq!(copy, TileMessage::decode(&copy.encode()).unwrap());
    assert_eq!(draw_mode, TileMessage::decode(&draw_mode.encode()).unwrap());
    assert_eq!(stats, TileMessage::decode(&stats.encode()).unwrap());
    assert_eq!(offset, TileMessage::decode(&offset.encode()).unwrap());
}

#[test]
fn tile_message_unknown_tag() {
    let err = TileMessage::decode(&[0xFF]).unwrap_err();
    assert!(matches!(
        err,
        FrameError::UnknownMessageType {
            channel: 0x0B,
            tag: 0xFF
        }
    ));
}
