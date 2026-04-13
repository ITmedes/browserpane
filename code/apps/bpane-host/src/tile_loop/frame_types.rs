//! Per-frame data types that flow between processing phases.

use crate::capture::ffmpeg::CaptureRegion;
use crate::cdp_video;
use crate::tiles;

/// CDP hint snapshot for the current frame.
#[derive(Debug, Clone)]
pub struct CdpFrameHints {
    pub video_region_hint: Option<CaptureRegion>,
    pub editable_region_hint: Option<CaptureRegion>,
    pub editable_qoi_tile_bounds: Option<(u16, u16, u16, u16)>,
    pub key_input_qoi_boost: bool,
    pub cdp_scroll_dy_px: Option<i16>,
    pub cdp_click_armed: bool,
    pub cdp_hint_tile_bounds: Option<(u16, u16, u16, u16)>,
    pub hint_snapshot: cdp_video::PageHintState,
}

/// Result of the two-pass video classification.
#[derive(Debug)]
pub struct ClassifyResult {
    pub latched_video_tiles: Vec<tiles::TileCoord>,
    pub cdp_motion_tiles: u32,
}

/// Result of scroll detection for the current frame.
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
