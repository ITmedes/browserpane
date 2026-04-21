//! Main run loop for the tile capture thread.
//!
//! The per-frame processing is delegated to methods in sibling files:
//! - `cdp_scroll.rs`: CDP hint processing and scroll detection
//! - `classify.rs`: two-pass video tile classification
//! - `scroll_emit.rs`: scroll residual analysis and dirty-set computation
//! - `emit.rs`: static/content split and tile emission
//! - `h264_region.rs`: H.264 region management
//! - `resize.rs`: screen resize handling

use tracing::warn;

use crate::capture::ffmpeg::CaptureRegion;
use crate::cdp_video::{HintRegionKind, PageHintState};
use crate::scroll::select_capture_frame_interval;

pub(super) fn should_force_refresh_for_video_hint_drop(
    active_region: Option<CaptureRegion>,
    hint: &PageHintState,
) -> bool {
    active_region.is_some()
        && (!hint.visible
            || !hint.focused
            || hint.region_kind != HintRegionKind::Video
            || hint.video_region.is_none())
}

impl super::TileCaptureThread {
    /// Run the tile capture loop. Blocks the current thread until the
    /// tile channel is closed or an unrecoverable error occurs.
    pub fn run(mut self) {
        // ── Constants ────────────────────────────────────────────
        const VIDEO_ENTER_SCORE: i8 = 5;
        const VIDEO_EXIT_SCORE: i8 = -1;
        const VIDEO_MAX_SCORE: i8 = 20;
        const VIDEO_DECAY_STREAK: u8 = 18;
        const VIDEO_MIN_HOLD_FRAMES: u8 = 12;
        const MIN_SCROLL_DY_PX: i32 = 2;
        const INPUT_MIN_SCROLL_DY_PX: i32 = 4;
        const MAX_CDP_SCROLL_DY_PX: i64 = crate::scroll::CONTENT_SCROLL_SEARCH_MAX_PX as i64;
        const CDP_CONTENT_DY_DIVERGENCE_LOG_PX: i32 = 3;
        const INPUT_SCROLL_MIN_CONFIDENCE: f32 = 0.80;
        const NO_INPUT_SCROLL_MIN_CONFIDENCE: f32 = 0.86;
        const SCROLL_SUPPRESS_VIDEO_FRAMES: u8 = 14;
        const SCROLL_RESIDUAL_FULL_REPAINT_RATIO: f32 =
            crate::scroll::SCROLL_RESIDUAL_FULL_REPAINT_RATIO_DEFAULT;
        const SCROLL_THIN_MODE_RESIDUAL_RATIO: f32 = 0.14;
        const SCROLL_THIN_REPAIR_QUIET_FRAMES: u8 = 5;
        const REGION_RECONFIG_STABLE_FRAMES: u8 = 2;
        const REGION_RECONFIG_MIN_INTERVAL_MS: u64 = 350;
        const REGION_MIN_CANDIDATES: u32 = 6;
        const REGION_DENSE_CANDIDATES: u32 = 18;
        const MIN_VIDEO_BBOX_WIDTH_TILES: u16 = 4;
        const MIN_VIDEO_BBOX_HEIGHT_TILES: u16 = 3;
        const MIN_VIDEO_BBOX_AREA_RATIO: f32 = 0.10;
        const EDITABLE_QOI_TILE_MARGIN: u16 = 2;
        const EDITABLE_HINT_HOLD_MS: u64 = 450;
        const KEY_INPUT_QOI_BOOST_MS: u64 = 800;
        const H264_MIN_ON_DURATION_MS: u64 = 900;

        // Send initial grid config
        let grid_frame = self.emitter.emit_grid_config(&self.grid);
        if self.tile_tx.blocking_send(grid_frame).is_err() {
            return;
        }

        loop {
            // ── Frame timing ─────────────────────────────────────
            let frame_interval = select_capture_frame_interval(
                self.base_frame_interval,
                self.scroll_active_frame_interval,
                self.scroll_active_capture_frames_remaining,
            );
            let now = std::time::Instant::now();
            let since_last = now.duration_since(self.last_capture);
            let sleep_dur = if since_last >= frame_interval {
                std::time::Duration::from_millis(16)
            } else {
                (frame_interval - since_last).max(std::time::Duration::from_millis(16))
            };
            std::thread::sleep(sleep_dur);

            // ── Cache miss handling ──────────────────────────────
            let mut force_refresh = false;
            while let Ok((frame_seq, col, row, hash)) = self.cache_miss_rx.try_recv() {
                self.emitter.handle_cache_miss(col, row, hash);
                self.client_cache_miss_reports_total =
                    self.client_cache_miss_reports_total.saturating_add(1);
                force_refresh = true;
                tracing::trace!(
                    frame_seq,
                    col,
                    row,
                    hash,
                    "tile cache miss reported by client"
                );
            }
            if !force_refresh {
                let hint = {
                    let guard = match self.browser_video_hint.lock() {
                        Ok(g) => g,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    *guard
                };
                if should_force_refresh_for_video_hint_drop(self.region_committer.active, &hint) {
                    force_refresh = true;
                }
            }

            // ── Damage gating ────────────────────────────────────
            let has_damage = match self.damage.as_mut() {
                Some(dt) => dt.poll(),
                None => true,
            };
            if !has_damage && !force_refresh {
                continue;
            }

            let now = std::time::Instant::now();
            if now.duration_since(self.last_capture) < frame_interval {
                continue;
            }
            self.last_capture = now;

            // ── Resize check ─────────────────────────────────────
            if self.handle_resize(now) {
                continue;
            }

            // ── Capture ──────────────────────────────────────────
            let raw = match self
                .cap
                .capture_region_raw(0, 0, self.screen_w, self.screen_h)
            {
                Ok(data) => data,
                Err(e) => {
                    warn!("tile capture: GetImage failed: {e}");
                    continue;
                }
            };
            let rgba = raw;
            self.grid.advance_frame();
            let stride = self.screen_w as usize * 4;

            // ── Phase 1: CDP hints + scroll detection ────────────
            let cdp = self.process_cdp_and_scroll(
                &rgba,
                stride,
                now,
                EDITABLE_HINT_HOLD_MS,
                KEY_INPUT_QOI_BOOST_MS,
                EDITABLE_QOI_TILE_MARGIN,
                MAX_CDP_SCROLL_DY_PX,
                CDP_CONTENT_DY_DIVERGENCE_LOG_PX,
                MIN_SCROLL_DY_PX,
                INPUT_MIN_SCROLL_DY_PX,
                INPUT_SCROLL_MIN_CONFIDENCE,
                NO_INPUT_SCROLL_MIN_CONFIDENCE,
                SCROLL_SUPPRESS_VIDEO_FRAMES,
            );

            // ── Phase 2: Video classification ────────────────────
            let classify = self.classify_tiles(
                &rgba,
                stride,
                cdp.cdp_video_region_hint,
                cdp.cdp_hint_tile_bounds,
                VIDEO_ENTER_SCORE,
                VIDEO_EXIT_SCORE,
                VIDEO_MAX_SCORE,
                VIDEO_DECAY_STREAK,
                VIDEO_MIN_HOLD_FRAMES,
                REGION_MIN_CANDIDATES,
                REGION_DENSE_CANDIDATES,
                MIN_VIDEO_BBOX_WIDTH_TILES,
                MIN_VIDEO_BBOX_HEIGHT_TILES,
                MIN_VIDEO_BBOX_AREA_RATIO,
            );

            // ── Phase 3: Scroll residual + dirty set ─────────────
            let scroll = self.compute_scroll_and_dirty(
                &rgba,
                stride,
                force_refresh,
                cdp.detected_scroll_frame,
                cdp.pending_scrolls,
                cdp.pending_scroll_dy_sum,
                cdp.input_scroll_dir,
                cdp.cdp_scroll_dy_px,
                SCROLL_RESIDUAL_FULL_REPAINT_RATIO,
                SCROLL_THIN_MODE_RESIDUAL_RATIO,
                SCROLL_THIN_REPAIR_QUIET_FRAMES,
            );

            // ── Phase 4: Tile emission ───────────────────────────
            if !self.emit_tiles(
                &rgba,
                stride,
                scroll.all_dirty,
                &scroll.detected_scroll_frame,
                &classify.latched_video_tiles,
                cdp.cdp_video_region_hint,
                classify.cdp_motion_tiles,
                cdp.editable_qoi_tile_bounds,
                cdp.key_input_qoi_boost,
                scroll.scroll_residual_ratio,
                scroll.scroll_residual_fallback_full,
                scroll.scroll_residual_tiles_frame,
                scroll.scroll_potential_tiles_frame,
                scroll.scroll_saved_tiles_frame,
                scroll.scroll_saved_ratio_frame,
                scroll.scroll_emit_ratio_frame,
                scroll.scroll_thin_mode_frame,
                scroll.scroll_thin_repair_frame,
            ) {
                return; // channel closed
            }

            // ── Phase 5: H.264 region management ─────────────────
            let cdp_has_video =
                cdp.cdp_video_region_hint.is_some() && self.scroll_cooldown_frames == 0;
            let _h264 = self.update_h264_region(
                cdp.cdp_video_region_hint,
                cdp_has_video,
                now,
                REGION_RECONFIG_STABLE_FRAMES,
                REGION_RECONFIG_MIN_INTERVAL_MS,
                H264_MIN_ON_DURATION_MS,
            );

            // ── Phase 6: Send frames + cleanup ───────────────────
            if let Some(dt) = self.damage.as_mut() {
                dt.reset();
            }
            self.prev_frame = Some(rgba);
        }
    }
}
