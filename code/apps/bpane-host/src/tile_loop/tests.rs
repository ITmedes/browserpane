use super::frame_types::*;
use crate::tiles;

// ── DetectedScrollFrame ─────────────────────────────────────────────

#[test]
fn detected_scroll_frame_fields_accessible() {
    let dsf = DetectedScrollFrame {
        dy: 64,
        confidence: 0.92,
        source: "content",
        direction_matches: true,
        min_confidence: Some(0.80),
        row_shift: 1,
        region_top: 0,
        region_bottom: 768,
        region_right: 1280,
    };
    assert_eq!(dsf.dy, 64);
    assert_eq!(dsf.source, "content");
    assert_eq!(dsf.row_shift, 1);
}

// ── CdpScrollResult ─────────────────────────────────────────────────

#[test]
fn cdp_scroll_result_no_scroll() {
    let result = CdpScrollResult {
        cdp_video_region_hint: None,
        cdp_hint_tile_bounds: None,
        editable_qoi_tile_bounds: None,
        key_input_qoi_boost: false,
        pending_scrolls: 0,
        pending_scroll_dy_sum: 0,
        input_scroll_dir: 0,
        cdp_scroll_dy_px: None,
        strong_scroll_observed: false,
        detected_scroll_frame: None,
    };
    assert!(result.detected_scroll_frame.is_none());
    assert_eq!(result.pending_scrolls, 0);
}

// ── ClassifyResult ──────────────────────────────────────────────────

#[test]
fn classify_result_empty_by_default() {
    let cr = ClassifyResult {
        latched_video_tiles: Vec::new(),
        cdp_motion_tiles: 0,
    };
    assert!(cr.latched_video_tiles.is_empty());
    assert_eq!(cr.cdp_motion_tiles, 0);
}

// ── ScrollEmitResult ────────────────────────────────────────────────

#[test]
fn scroll_emit_result_no_scroll() {
    let result = ScrollEmitResult {
        all_dirty: vec![tiles::TileCoord::new(0, 0), tiles::TileCoord::new(1, 0)],
        detected_scroll_frame: None,
        scroll_residual_ratio: None,
        scroll_residual_fallback_full: false,
        scroll_residual_tiles_frame: None,
        scroll_potential_tiles_frame: None,
        scroll_saved_tiles_frame: None,
        scroll_saved_ratio_frame: None,
        scroll_emit_ratio_frame: None,
        scroll_thin_mode_frame: false,
        scroll_thin_repair_frame: false,
    };
    assert_eq!(result.all_dirty.len(), 2);
    assert!(!result.scroll_residual_fallback_full);
}

// ── H264Update ──────────────────────────────────────────────────────

#[test]
fn h264_update_struct_accessible() {
    let update = super::h264_region::H264Update {
        tiles_cover_screen: true,
    };
    assert!(update.tiles_cover_screen);
}
