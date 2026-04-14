use super::frame_types::*;
use crate::capture::ffmpeg::CaptureRegion;
use crate::cdp_video::{HintRegionKind, PageHintState};
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

// ── should_force_refresh_for_video_hint_drop ───────────────────────

#[test]
fn force_refresh_when_active_video_hint_disappears() {
    let hint = PageHintState {
        visible: true,
        focused: true,
        region_kind: HintRegionKind::Video,
        video_region: None,
        ..PageHintState::default()
    };
    assert!(super::run::should_force_refresh_for_video_hint_drop(
        Some(CaptureRegion {
            x: 0,
            y: 0,
            w: 640,
            h: 360,
        }),
        &hint,
    ));
}

#[test]
fn no_force_refresh_while_video_hint_still_present() {
    let hint = PageHintState {
        visible: true,
        focused: true,
        region_kind: HintRegionKind::Video,
        video_region: Some(CaptureRegion {
            x: 0,
            y: 0,
            w: 640,
            h: 360,
        }),
        ..PageHintState::default()
    };
    assert!(!super::run::should_force_refresh_for_video_hint_drop(
        Some(CaptureRegion {
            x: 0,
            y: 0,
            w: 640,
            h: 360,
        }),
        &hint,
    ));
}

// ── arbitrate_scroll_trust ──────────────────────────────────────────

#[test]
fn trust_prefers_content_when_cdp_and_content_agree() {
    let result = super::scroll_trust::arbitrate_scroll_trust(
        Some(64),                           // cdp_scroll_dy_px
        Some((60, 0.90, true, Some(0.80))), // content_scroll
        1,                                  // pending_scrolls (wheel active)
        -64,                                // pending_scroll_dy_sum
        1,                                  // input_scroll_dir
        1,                                  // cdp_scroll_dir
        2,                                  // min_scroll_dy_px
        3,                                  // cdp_content_dy_divergence_log_px
    );
    let (dy, _, source, _, _) = result.unwrap();
    assert_eq!(source, "content");
    assert_eq!(dy, 60); // content's pixel-aligned value
}

#[test]
fn trust_falls_back_to_cdp_when_content_disagrees_direction() {
    let result = super::scroll_trust::arbitrate_scroll_trust(
        Some(64),
        Some((-30, 0.92, false, Some(0.86))), // direction_matches=false
        1,
        -64,
        1,
        1,
        2,
        3,
    );
    let (dy, _, source, _, _) = result.unwrap();
    assert_eq!(source, "cdp");
    assert_eq!(dy, 64);
}

#[test]
fn trust_returns_none_when_cdp_too_small() {
    let result = super::scroll_trust::arbitrate_scroll_trust(
        Some(1), // below min_scroll_dy_px=2
        None,
        0,
        0,
        0,
        0,
        2, // min_scroll_dy_px
        3,
    );
    assert!(result.is_none());
}

#[test]
fn trust_uses_content_when_no_cdp() {
    let result = super::scroll_trust::arbitrate_scroll_trust(
        None,                               // no CDP
        Some((32, 0.88, true, Some(0.80))), // content available
        0,
        0,
        0,
        0,
        2,
        3,
    );
    let (dy, _, source, _, _) = result.unwrap();
    assert_eq!(source, "content");
    assert_eq!(dy, 32);
}

#[test]
fn trust_returns_none_when_nothing_detected() {
    let result = super::scroll_trust::arbitrate_scroll_trust(None, None, 0, 0, 0, 0, 2, 3);
    assert!(result.is_none());
}

#[test]
fn trust_passive_cdp_prefers_content_when_directions_agree() {
    // No pending_scrolls, but CDP + content both present with same direction
    let result = super::scroll_trust::arbitrate_scroll_trust(
        Some(64),                           // cdp
        Some((60, 0.90, true, Some(0.80))), // content, same direction
        0,                                  // no pending_scrolls
        0,
        0,
        1, // cdp_scroll_dir
        2,
        3,
    );
    let (dy, _, source, _, _) = result.unwrap();
    assert_eq!(source, "content");
    assert_eq!(dy, 60);
}

#[test]
fn trust_passive_cdp_skipped_when_content_absent() {
    // CDP present but no content confirmation → skip (mid-render)
    let result = super::scroll_trust::arbitrate_scroll_trust(
        Some(64),
        None, // no content
        0,    // no pending_scrolls
        0,
        0,
        1,
        2,
        3,
    );
    assert!(result.is_none(), "should skip unconfirmed CDP");
}

// ── partition_and_compare ───────────────────────────────────────────

#[test]
fn partition_splits_content_and_chrome() {
    let grid = tiles::TileGrid::new(128, 128, 64);
    let frame = vec![0u8; 128 * 128 * 4];
    let stride = 128 * 4;
    let emit_coords: Vec<tiles::TileCoord> = (0..grid.rows)
        .flat_map(|r| (0..grid.cols).map(move |c| tiles::TileCoord::new(c, r)))
        .collect();
    let dsf = DetectedScrollFrame {
        dy: 64,
        confidence: 0.95,
        source: "content",
        direction_matches: true,
        min_confidence: Some(0.80),
        row_shift: 1,
        region_top: 64, // chrome header = top 64px (row 0)
        region_bottom: 128,
        region_right: 128,
    };

    let result = super::scroll_partition::partition_and_compare(
        &frame,
        &frame,
        stride,
        &grid,
        0,
        64,
        64,
        64,
        &emit_coords,
        &dsf,
        0,
        128,
        128,
        128,
    );

    // Row 0 is chrome (top 64px), row 1 is content
    assert!(
        result
            .chrome_emit_coords
            .contains(&tiles::TileCoord::new(0, 0)),
        "row 0 should be chrome"
    );
    assert!(
        result
            .content_emit_coords
            .contains(&tiles::TileCoord::new(0, 1)),
        "row 1 should be content"
    );
}

#[test]
fn partition_no_split_full_viewport() {
    let grid = tiles::TileGrid::new(128, 128, 64);
    let frame = vec![0u8; 128 * 128 * 4];
    let stride = 128 * 4;
    let emit_coords: Vec<tiles::TileCoord> = (0..grid.rows)
        .flat_map(|r| (0..grid.cols).map(move |c| tiles::TileCoord::new(c, r)))
        .collect();
    let dsf = DetectedScrollFrame {
        dy: 64,
        confidence: 0.95,
        source: "content",
        direction_matches: true,
        min_confidence: Some(0.80),
        row_shift: 1,
        region_top: 0,
        region_bottom: 128,
        region_right: 128,
    };

    let result = super::scroll_partition::partition_and_compare(
        &frame,
        &frame,
        stride,
        &grid,
        0,
        64,
        64,
        64,
        &emit_coords,
        &dsf,
        0,
        128,
        128,
        128,
    );

    // Full viewport → no chrome
    assert!(result.chrome_emit_coords.is_empty());
    assert_eq!(result.content_emit_coords.len(), emit_coords.len());
}

#[test]
fn partition_identical_frames_have_zero_residual() {
    let grid = tiles::TileGrid::new(128, 128, 64);
    let frame = vec![42u8; 128 * 128 * 4];
    let stride = 128 * 4;
    let emit_coords: Vec<tiles::TileCoord> = (0..grid.rows)
        .flat_map(|r| (0..grid.cols).map(move |c| tiles::TileCoord::new(c, r)))
        .collect();
    let dsf = DetectedScrollFrame {
        dy: 0,
        confidence: 0.95,
        source: "content",
        direction_matches: true,
        min_confidence: Some(0.80),
        row_shift: 0,
        region_top: 0,
        region_bottom: 128,
        region_right: 128,
    };

    let result = super::scroll_partition::partition_and_compare(
        &frame,
        &frame,
        stride,
        &grid,
        0,
        64,
        64,
        0,
        &emit_coords,
        &dsf,
        0,
        128,
        128,
        128,
    );

    // dy=0 → residual == full set (no scroll optimization)
    assert_eq!(result.residual.len(), result.content_emit_coords.len());
}
