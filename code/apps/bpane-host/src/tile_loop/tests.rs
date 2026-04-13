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
