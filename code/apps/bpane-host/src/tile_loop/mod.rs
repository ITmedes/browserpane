//! Tile capture thread: captures X11 screen, classifies tiles, detects
//! scroll, emits lossless tile frames alongside H.264 video.
//!
//! The `TileCaptureThread` struct replaces the 1500-line spawn_blocking
//! closure with structured state and delegated per-frame processing.

mod cdp_hints;
mod cdp_scroll;
mod cdp_scroll_track;
mod classify;
mod dirty_set;
mod emit;
pub(crate) mod frame_types;
mod h264_region;
mod resize;
mod run;
mod scroll_emit;
mod scroll_partition;
mod scroll_residual;
mod scroll_resolve;
mod scroll_send;
mod scroll_trust;

#[cfg(test)]
mod tests;

use std::sync::atomic::{AtomicBool, AtomicU64};
use std::sync::{Arc, Mutex};

use tokio::sync::mpsc as tokio_mpsc;
use tracing::{debug, info, warn};

use bpane_protocol::frame::Frame;
use bpane_protocol::VideoTileInfo;

use crate::capture;
use crate::capture::CaptureBackend;
use crate::cdp_video;
use crate::config::{H264Mode, TileCaptureConfig};
use crate::tiles;
use crate::video_region;

/// All state for the tile capture thread, previously ~40 local variables
/// inside a `spawn_blocking` closure.
pub struct TileCaptureThread {
    // ── Config (read-only after init) ────────────────────────────────
    pub(crate) h264_mode: H264Mode,
    pub(crate) tile_size: u16,
    pub(crate) tile_codec: tiles::emitter::TileCodec,
    pub(crate) video_classification_enabled: bool,
    pub(crate) scroll_copy_quantum_px: u16,
    pub(crate) base_frame_interval: std::time::Duration,
    pub(crate) scroll_active_frame_interval: std::time::Duration,
    pub(crate) scroll_active_capture_frames: u8,
    pub(crate) min_cdp_video_width_px: u32,
    pub(crate) min_cdp_video_height_px: u32,
    pub(crate) min_cdp_video_area_ratio: f32,
    pub(crate) cdp_video_tile_margin: u16,
    pub(crate) scroll_thin_mode_enabled: bool,

    // ── Capture ──────────────────────────────────────────────────────
    pub(crate) cap: capture::x11::X11CaptureBackend,
    pub(crate) damage: Option<capture::x11::DamageTracker>,
    pub(crate) screen_w: u16,
    pub(crate) screen_h: u16,

    // ── Grid & emitter ───────────────────────────────────────────────
    pub(crate) grid: tiles::TileGrid,
    pub(crate) emitter: tiles::emitter::TileEmitter,

    // ── Scroll state ─────────────────────────────────────────────────
    pub(crate) prev_frame: Option<Vec<u8>>,
    pub(crate) content_origin_y: i64,
    pub(crate) grid_offset_y: u16,
    pub(crate) scroll_active_capture_frames_remaining: u8,
    pub(crate) last_cdp_hint_seq: u64,
    pub(crate) last_cdp_scroll_y: Option<i64>,
    pub(crate) cdp_scroll_anchor: Option<(i64, i64)>,
    pub(crate) scroll_cooldown_frames: u8,
    pub(crate) scroll_residual_batches_total: u64,
    pub(crate) scroll_residual_fallback_full_total: u64,
    pub(crate) scroll_fallback_non_quantized_total: u64,
    pub(crate) scroll_fallback_residual_full_repaint_total: u64,
    pub(crate) scroll_zero_saved_batches_total: u64,
    pub(crate) scroll_potential_tiles_total: u64,
    pub(crate) scroll_residual_tiles_total: u64,
    pub(crate) scroll_saved_tiles_total: u64,
    pub(crate) client_cache_miss_reports_total: u64,
    pub(crate) scroll_thin_mode_active: bool,
    pub(crate) scroll_residual_was_active: bool,
    pub(crate) scroll_quiet_frames: u8,
    pub(crate) scroll_thin_batches_total: u64,
    pub(crate) scroll_thin_repairs_total: u64,
    pub(crate) last_scroll_region_top: u16,
    pub(crate) last_scroll_region_bottom: u16,
    pub(crate) last_scroll_region_right: u16,

    // ── Video classification state ───────────────────────────────────
    pub(crate) prev_hashes: Vec<u64>,
    pub(crate) video_scores: Vec<i8>,
    pub(crate) non_candidate_streaks: Vec<u8>,
    pub(crate) video_hold_frames: Vec<u8>,
    pub(crate) video_latched: Vec<bool>,
    pub(crate) candidate_mask: Vec<bool>,
    pub(crate) changed_mask: Vec<bool>,
    pub(crate) text_like_mask: Vec<bool>,
    pub(crate) prev_video_bbox: Option<(u16, u16, u16, u16)>,
    pub(crate) stable_bbox_frames: u8,
    pub(crate) editable_hint: video_region::EditableHintState,

    // ── H264 / region management ─────────────────────────────────────
    pub(crate) h264_enabled: bool,
    pub(crate) region_committer: video_region::RegionCommitter,
    pub(crate) last_h264_toggle_at: std::time::Instant,

    // ── Channels & shared state ──────────────────────────────────────
    pub(crate) tile_tx: tokio_mpsc::Sender<Frame>,
    pub(crate) cmd_tx: std::sync::mpsc::Sender<capture::ffmpeg::PipelineCmd>,
    pub(crate) tiles_active: Arc<AtomicBool>,
    pub(crate) video_tile_info: Arc<Mutex<Option<VideoTileInfo>>>,
    pub(crate) browser_video_hint: Arc<Mutex<cdp_video::PageHintState>>,
    pub(crate) input_activity: Arc<AtomicU64>,
    pub(crate) scroll_rx: std::sync::mpsc::Receiver<(i16, i16)>,
    pub(crate) text_input_rx: std::sync::mpsc::Receiver<std::time::Instant>,
    pub(crate) cache_miss_rx: std::sync::mpsc::Receiver<(u32, u16, u16, u64)>,

    // ── Timing ───────────────────────────────────────────────────────
    pub(crate) last_capture: std::time::Instant,
    pub(crate) last_resize_check: std::time::Instant,
}

impl TileCaptureThread {
    /// Construct the tile capture thread. Initialises X11 capture and grid.
    /// Returns `None` if the X11 backend fails to initialise.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        display: &str,
        width: u32,
        height: u32,
        config: TileCaptureConfig,
        tile_tx: tokio_mpsc::Sender<Frame>,
        cmd_tx: std::sync::mpsc::Sender<capture::ffmpeg::PipelineCmd>,
        tiles_active: Arc<AtomicBool>,
        video_tile_info: Arc<Mutex<Option<VideoTileInfo>>>,
        browser_video_hint: Arc<Mutex<cdp_video::PageHintState>>,
        input_activity: Arc<AtomicU64>,
        scroll_rx: std::sync::mpsc::Receiver<(i16, i16)>,
        text_input_rx: std::sync::mpsc::Receiver<std::time::Instant>,
        cache_miss_rx: std::sync::mpsc::Receiver<(u32, u16, u16, u64)>,
    ) -> Option<Self> {
        let TileCaptureConfig {
            h264_mode,
            tile_size,
            tile_codec,
            scroll_copy_quantum_px,
            base_frame_interval,
            scroll_active_frame_interval,
            scroll_active_capture_frames,
            min_cdp_video_width_px,
            min_cdp_video_height_px,
            min_cdp_video_area_ratio,
            cdp_video_tile_margin,
            scroll_thin_mode_enabled,
            video_classification_enabled,
            ..
        } = config;

        let cap = match capture::x11::X11CaptureBackend::new(display, width, height) {
            Ok(c) => c,
            Err(e) => {
                warn!("tile capture: X11 backend init failed: {e}");
                return None;
            }
        };

        let (init_w, init_h) = cap.resolution();
        let screen_w = init_w as u16;
        let screen_h = init_h as u16;
        info!(
            "tile capture: active ({}x{}, tile_size={}, codec={:?})",
            screen_w, screen_h, tile_size, tile_codec
        );

        let grid = tiles::TileGrid::new(screen_w, screen_h, tile_size);
        let emitter = tiles::emitter::TileEmitter::with_codec(grid.cols, grid.rows, tile_codec);
        let total_tiles = grid.cols as usize * grid.rows as usize;

        let damage = capture::x11::DamageTracker::with_options(
            display,
            None,
            None,
            Some(input_activity.clone()),
        )
        .ok()
        .flatten();
        if damage.is_some() {
            debug!("tile capture: XDamage tracking active");
        }

        let now = std::time::Instant::now();
        Some(Self {
            h264_mode,
            tile_size,
            tile_codec,
            video_classification_enabled,
            scroll_copy_quantum_px,
            base_frame_interval,
            scroll_active_frame_interval,
            scroll_active_capture_frames,
            min_cdp_video_width_px,
            min_cdp_video_height_px,
            min_cdp_video_area_ratio,
            cdp_video_tile_margin,
            scroll_thin_mode_enabled,
            cap,
            damage,
            screen_w,
            screen_h,
            grid,
            emitter,
            prev_frame: None,
            content_origin_y: 0,
            grid_offset_y: 0,
            scroll_active_capture_frames_remaining: 0,
            last_cdp_hint_seq: 0,
            last_cdp_scroll_y: None,
            cdp_scroll_anchor: None,
            scroll_cooldown_frames: 0,
            scroll_residual_batches_total: 0,
            scroll_residual_fallback_full_total: 0,
            scroll_fallback_non_quantized_total: 0,
            scroll_fallback_residual_full_repaint_total: 0,
            scroll_zero_saved_batches_total: 0,
            scroll_potential_tiles_total: 0,
            scroll_residual_tiles_total: 0,
            scroll_saved_tiles_total: 0,
            client_cache_miss_reports_total: 0,
            scroll_thin_mode_active: false,
            scroll_residual_was_active: false,
            scroll_quiet_frames: 0,
            scroll_thin_batches_total: 0,
            scroll_thin_repairs_total: 0,
            last_scroll_region_top: 0,
            last_scroll_region_bottom: screen_h,
            last_scroll_region_right: screen_w,
            prev_hashes: vec![0; total_tiles],
            video_scores: vec![0; total_tiles],
            non_candidate_streaks: vec![0; total_tiles],
            video_hold_frames: vec![0; total_tiles],
            video_latched: vec![false; total_tiles],
            candidate_mask: vec![false; total_tiles],
            changed_mask: vec![false; total_tiles],
            text_like_mask: vec![false; total_tiles],
            prev_video_bbox: None,
            stable_bbox_frames: 0,
            editable_hint: video_region::EditableHintState::new(),
            h264_enabled: h264_mode.starts_enabled(),
            region_committer: video_region::RegionCommitter::new(),
            last_h264_toggle_at: now,
            tile_tx,
            cmd_tx,
            tiles_active,
            video_tile_info,
            browser_video_hint,
            input_activity,
            scroll_rx,
            text_input_rx,
            cache_miss_rx,
            last_capture: now,
            last_resize_check: now - std::time::Duration::from_secs(1),
        })
    }
}
