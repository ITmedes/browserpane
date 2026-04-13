//! Per-frame data types that flow between processing phases.

use crate::capture::ffmpeg::CaptureRegion;
use crate::cdp_video;
use crate::tiles;

/// Snapshot of CDP hints after draining input events: video/editable
/// region hints, editable QOI tile bounds, and pending scroll counts.
#[derive(Debug)]
pub struct CdpHintSnapshot {
    pub cdp_hint_snapshot: cdp_video::PageHintState,
    pub cdp_video_region_hint_sized: Option<CaptureRegion>,
    pub cdp_editable_region_hint: Option<CaptureRegion>,
    pub editable_qoi_tile_bounds: Option<(u16, u16, u16, u16)>,
    pub key_input_qoi_boost: bool,
    pub pending_scrolls: i32,
    pub pending_scroll_dy_sum: i32,
}

/// Output of CDP hint + scroll detection phase.
#[derive(Debug)]
pub struct CdpScrollResult {
    pub cdp_video_region_hint: Option<CaptureRegion>,
    pub cdp_hint_tile_bounds: Option<(u16, u16, u16, u16)>,
    pub editable_qoi_tile_bounds: Option<(u16, u16, u16, u16)>,
    pub key_input_qoi_boost: bool,
    pub pending_scrolls: i32,
    pub pending_scroll_dy_sum: i32,
    pub input_scroll_dir: i32,
    pub cdp_scroll_dy_px: Option<i16>,
    pub strong_scroll_observed: bool,
    pub detected_scroll_frame: Option<DetectedScrollFrame>,
}

/// Result of the two-pass video classification.
#[derive(Debug)]
pub struct ClassifyResult {
    pub latched_video_tiles: Vec<tiles::TileCoord>,
    pub cdp_motion_tiles: u32,
}

/// Result of scroll residual analysis + dirty set computation.
#[derive(Debug)]
pub struct ScrollEmitResult {
    pub all_dirty: Vec<tiles::TileCoord>,
    pub detected_scroll_frame: Option<DetectedScrollFrame>,
    pub scroll_residual_ratio: Option<f32>,
    pub scroll_residual_fallback_full: bool,
    pub scroll_residual_tiles_frame: Option<usize>,
    pub scroll_potential_tiles_frame: Option<usize>,
    pub scroll_saved_tiles_frame: Option<usize>,
    pub scroll_saved_ratio_frame: Option<f32>,
    pub scroll_emit_ratio_frame: Option<f32>,
    pub scroll_thin_mode_frame: bool,
    pub scroll_thin_repair_frame: bool,
}

/// A detected scroll displacement for the current frame.
#[derive(Debug, Clone)]
pub struct DetectedScrollFrame {
    pub dy: i16,
    pub confidence: f32,
    pub source: &'static str,
    pub direction_matches: bool,
    pub min_confidence: Option<f32>,
    pub row_shift: i32,
    pub region_top: u16,
    pub region_bottom: u16,
    pub region_right: u16,
}
