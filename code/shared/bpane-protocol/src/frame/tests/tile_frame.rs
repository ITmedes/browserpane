use alloc::vec;

use crate::{ChannelId, TileMessage};

use super::super::{Frame, Message};

#[test]
fn tile_message_to_frame_round_trip() {
    let msg = TileMessage::Fill {
        col: 3,
        row: 7,
        rgba: 0xFFFF_FFFF,
    };
    let frame = msg.to_frame();
    assert_eq!(frame.channel, ChannelId::Tiles);
    match Message::from_frame(&frame).unwrap() {
        Message::Tiles(decoded) => assert_eq!(decoded, msg),
        _ => panic!("expected Tiles message"),
    }
}

#[test]
fn tile_all_variants_via_frame() {
    let messages = vec![
        TileMessage::GridConfig {
            tile_size: 64,
            cols: 20,
            rows: 12,
            screen_w: 1280,
            screen_h: 768,
        },
        TileMessage::CacheHit {
            col: 0,
            row: 0,
            hash: 0,
        },
        TileMessage::CacheMiss {
            frame_seq: 1,
            col: 0,
            row: 0,
            hash: 0,
        },
        TileMessage::Fill {
            col: 1,
            row: 1,
            rgba: 0,
        },
        TileMessage::Qoi {
            col: 2,
            row: 2,
            hash: 99,
            data: vec![1, 2, 3],
        },
        TileMessage::Zstd {
            col: 4,
            row: 3,
            hash: 123,
            data: vec![0x28, 0xB5, 0x2F, 0xFD],
        },
        TileMessage::VideoRegion {
            x: 0,
            y: 0,
            w: 100,
            h: 100,
        },
        TileMessage::BatchEnd { frame_seq: 0 },
        TileMessage::ScrollCopy {
            dx: -64,
            dy: 128,
            region_top: 0,
            region_bottom: 768,
            region_right: 1280,
        },
        TileMessage::GridOffset {
            offset_x: 8,
            offset_y: -8,
        },
        TileMessage::TileDrawMode {
            apply_offset: false,
        },
        TileMessage::ScrollStats {
            scroll_batches_total: 3,
            scroll_full_fallbacks_total: 1,
            scroll_potential_tiles_total: 100,
            scroll_saved_tiles_total: 72,
            scroll_non_quantized_fallbacks_total: 1,
            scroll_residual_full_repaints_total: 0,
            scroll_residual_interior_limit_fallbacks_total: 0,
            scroll_residual_low_saved_ratio_fallbacks_total: 0,
            scroll_residual_large_row_shift_fallbacks_total: 0,
            scroll_residual_other_fallbacks_total: 0,
            scroll_zero_saved_batches_total: 2,
            scroll_split_region_batches_total: 1,
            scroll_sticky_band_batches_total: 1,
            scroll_chrome_tiles_total: 9,
            scroll_exposed_strip_tiles_total: 6,
            scroll_interior_residual_tiles_total: 12,
            host_sent_hash_entries: 64,
            host_sent_hash_evictions_total: 5,
            host_cache_miss_reports_total: 3,
        },
    ];

    for msg in messages {
        let encoded = msg.to_frame().encode();
        let (decoded_frame, consumed) = Frame::decode(&encoded).unwrap();
        assert_eq!(consumed, encoded.len());
        match Message::from_frame(&decoded_frame).unwrap() {
            Message::Tiles(decoded) => assert_eq!(decoded, msg),
            _ => panic!("expected Tiles message for {msg:?}"),
        }
    }
}
