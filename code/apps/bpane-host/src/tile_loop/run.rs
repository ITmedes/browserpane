//! Main run loop for the tile capture thread.
//!
//! This file contains the frame processing loop that was previously
//! a 1500-line closure inside `run_ffmpeg_session`.

use std::sync::atomic::Ordering;

use tracing::{debug, trace, warn};

use bpane_protocol::VideoTileInfo;

use crate::capture::CaptureBackend;
use crate::cdp_video;
use crate::config::H264Mode;
use crate::region::{
    capture_region_tile_bounds, clamp_region_to_screen, expand_tile_bounds,
    extend_dirty_with_tile_bounds, region_meets_editable_minimum,
    region_meets_video_minimum, scale_css_px_to_screen_px,
};
use crate::scroll::{
    build_scroll_exposed_strip_emit_coords, build_scroll_residual_emit_coords,
    can_emit_scroll_copy, content_scroll_search_limit_px, detect_column_scroll,
    has_scroll_region_split, is_content_tile_in_scroll_region, is_scroll_delta_quantized,
    next_scroll_active_capture_frames, offset_tile_rect_for_emit,
    select_capture_frame_interval, select_wheel_trusted_scroll, should_defer_scroll_repair,
    should_emit_scroll_copy, tile_matches_shifted_prev,
};
use crate::tiles;
use crate::video_classify::{
    bbox_center_shift, bbox_iou, compute_tile_motion_features, is_photo_like_tile,
};
use crate::region::hash_tile_region;

impl super::TileCaptureThread {
    /// Run the tile capture loop. Blocks the current thread until the
    /// tile channel is closed or an unrecoverable error occurs.
    pub fn run(mut self) {
    // ── Constants ────────────────────────────────────────────────
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
    const SCROLL_RESIDUAL_FULL_REPAINT_RATIO: f32 = crate::scroll::SCROLL_RESIDUAL_FULL_REPAINT_RATIO_DEFAULT;
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
    const MIN_CHANGED_VIDEO_TILES_FOR_H264: u32 = 8;
    const CLICK_LATCH_RESET_FRAMES: u8 = 20;
    const H264_MIN_ON_DURATION_MS: u64 = 900;

    // Send self.grid config so the client knows the tile layout.
    let grid_frame = self.emitter.emit_grid_config(&self.grid);
    if self.tile_tx.blocking_send(grid_frame).is_err() {
        return;
    }
    loop {
        let frame_interval = select_capture_frame_interval(
            self.base_frame_interval,
            self.scroll_active_frame_interval,
            self.scroll_active_capture_frames_remaining,
        );
        // Sleep until next frame interval is due (minimum 16ms for event coalescing).
        let now = std::time::Instant::now();
        let since_last = now.duration_since(self.last_capture);
        let sleep_dur = if since_last >= frame_interval {
            std::time::Duration::from_millis(16)
        } else {
            (frame_interval - since_last).max(std::time::Duration::from_millis(16))
        };
        std::thread::sleep(sleep_dur);

        let mut force_refresh = false;
        while let Ok((frame_seq, col, row, hash)) = self.cache_miss_rx.try_recv() {
            self.emitter.handle_cache_miss(col, row, hash);
            force_refresh = true;
            trace!(
                frame_seq,
                col,
                row,
                hash,
                "tile cache miss reported by client"
            );
        }

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

        if self.handle_resize(now) {
            continue;
        }

        let raw = match self.cap.capture_region_raw(0, 0, self.screen_w, self.screen_h) {
            Ok(data) => data,
            Err(e) => {
                warn!("tile capture: GetImage failed: {e}");
                continue;
            }
        };

        // Pixels stay in native BGRA format — hashing is format-agnostic,
        // and the per-tile BGRA→RGBA swap happens only for QOI-encoded tiles
        // in the self.emitter (a small subset of total pixels).
        let rgba = raw;

        self.grid.advance_frame();
        let stride = self.screen_w as usize * 4;

        // ── Scroll displacement detection ─────────────────────
        // Drain pending scroll events (used only for logging).
        let mut pending_scrolls = 0i32;
        let mut pending_scroll_dy_sum = 0i32;
        while let Ok((_, dy)) = self.scroll_rx.try_recv() {
            pending_scrolls += 1;
            pending_scroll_dy_sum += dy as i32;
        }
        while let Ok((x, y, ts)) = self.video_click_rx.try_recv() {
            self.last_left_click = Some((x, y, ts));
        }
        while let Ok(ts) = self.text_input_rx.try_recv() {
            self.editable_hint.on_key_input(ts);
        }

        let prev_for_analysis = self.prev_frame.as_deref();
        let cdp_hint_snapshot = {
            let guard = match self.browser_video_hint.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            *guard
        };
        let cdp_hint_region_kind = cdp_hint_snapshot.region_kind;
        let cdp_hint_region_raw = cdp_hint_snapshot.video_region.and_then(|region| {
            clamp_region_to_screen(region, self.screen_w as u32, self.screen_h as u32)
        });
        let cdp_video_region_hint_sized =
            if matches!(cdp_hint_region_kind, cdp_video::HintRegionKind::Video) {
                cdp_hint_region_raw.filter(|region| {
                    region_meets_video_minimum(
                        region.w,
                        region.h,
                        self.screen_w as u32,
                        self.screen_h as u32,
                        self.min_cdp_video_width_px,
                        self.min_cdp_video_height_px,
                        self.min_cdp_video_area_ratio,
                    )
                })
            } else {
                None
            };
        let cdp_editable_region_hint =
            if matches!(cdp_hint_region_kind, cdp_video::HintRegionKind::Editable) {
                cdp_hint_region_raw
                    .filter(|region| region_meets_editable_minimum(region.w, region.h))
            } else {
                None
            };
        self.editable_hint.update(cdp_editable_region_hint, now, EDITABLE_HINT_HOLD_MS);
        let key_input_qoi_boost = self.editable_hint.key_input_qoi_boost(now, KEY_INPUT_QOI_BOOST_MS);
        let editable_qoi_region = self.editable_hint.qoi_region(
            cdp_editable_region_hint,
            now,
            EDITABLE_HINT_HOLD_MS,
            KEY_INPUT_QOI_BOOST_MS,
        );
        let editable_qoi_tile_bounds = editable_qoi_region.map(|region| {
            expand_tile_bounds(
                capture_region_tile_bounds(region, self.tile_size, self.grid.cols, self.grid.rows),
                EDITABLE_QOI_TILE_MARGIN,
                self.grid.cols,
                self.grid.rows,
            )
        });
        let mut cdp_scroll_dy_px: Option<i16> = None;
        let cdp_scale_milli = cdp_hint_snapshot.device_scale_factor_milli.max(1);
        if let Some(scroll_y_css) = cdp_hint_snapshot.scroll_y {
            let scroll_y = scale_css_px_to_screen_px(scroll_y_css, cdp_scale_milli);
            if let Some(prev_scroll_y) = self.last_cdp_scroll_y {
                let dy = scroll_y.saturating_sub(prev_scroll_y);
                if dy != 0 {
                    let clamped = dy.clamp(-MAX_CDP_SCROLL_DY_PX, MAX_CDP_SCROLL_DY_PX) as i16;
                    if clamped != 0 {
                        cdp_scroll_dy_px = Some(clamped);
                    }
                }
            }
            self.last_cdp_scroll_y = Some(scroll_y);
        } else {
            self.last_cdp_scroll_y = None;
        }
        if cdp_scroll_dy_px.is_none()
            && cdp_hint_snapshot.update_seq != 0
            && cdp_hint_snapshot.update_seq != self.last_cdp_hint_seq
            && cdp_hint_snapshot.scroll_delta_y != 0
        {
            let clamped = scale_css_px_to_screen_px(
                cdp_hint_snapshot.scroll_delta_y as i64,
                cdp_scale_milli,
            )
            .clamp(-MAX_CDP_SCROLL_DY_PX, MAX_CDP_SCROLL_DY_PX)
                as i16;
            if clamped != 0 {
                cdp_scroll_dy_px = Some(clamped);
            }
        }
        if cdp_hint_snapshot.update_seq != 0 {
            self.last_cdp_hint_seq = cdp_hint_snapshot.update_seq;
        }

        // Content-based scroll detection: compare current frame to
        // previous using vertical column matching. Runs on every frame
        // (not gated on input events) so it catches all scroll sources:
        // mouse wheel, scrollbar drag, keyboard, programmatic scrolls.
        // Industry standard (RDP/SPICE/VNC) intercepts OS drawing
        // commands; since we lack compositor hooks, framebuffer
        // comparison is the correct alternative.
        let mut strong_scroll_observed = false;
        let mut detected_scroll_dy_px: Option<i16> = None;
        let input_scroll_dir = (-pending_scroll_dy_sum).signum();
        let cdp_scroll_dir = cdp_scroll_dy_px.map(|dy| (dy as i32).signum()).unwrap_or(0);
        let hint_scroll_dir = if input_scroll_dir != 0 {
            input_scroll_dir
        } else {
            cdp_scroll_dir
        };
        let min_scroll_dy_px = if pending_scrolls > 0 {
            INPUT_MIN_SCROLL_DY_PX
        } else {
            MIN_SCROLL_DY_PX
        };
        let mut content_scroll: Option<(i16, f32, bool, Option<f32>)> = None;
        let content_scroll_search_px = content_scroll_search_limit_px(cdp_scroll_dy_px);
        if let Some(prev) = prev_for_analysis {
            if let Some((detected_dy, confidence)) = detect_column_scroll(
                &rgba,
                prev,
                stride,
                self.screen_w as usize,
                self.screen_h as usize,
                content_scroll_search_px,
            ) {
                let detected_scroll_dir = detected_dy.signum();
                let direction_matches = hint_scroll_dir == 0
                    || detected_scroll_dir == 0
                    || hint_scroll_dir == detected_scroll_dir;
                // Scroll hints are soft: matching direction lowers confidence threshold.
                let min_confidence = if hint_scroll_dir != 0 && direction_matches {
                    INPUT_SCROLL_MIN_CONFIDENCE
                } else {
                    NO_INPUT_SCROLL_MIN_CONFIDENCE
                };
                let trusted =
                    detected_dy.abs() >= min_scroll_dy_px && confidence >= min_confidence;
                if trusted {
                    content_scroll = Some((
                        detected_dy as i16,
                        confidence,
                        direction_matches,
                        Some(min_confidence),
                    ));
                } else {
                    trace!(
                        source = "content",
                        dy = detected_dy,
                        confidence = format!("{:.2}", confidence),
                        scrolls = pending_scrolls,
                        input_dy_sum = pending_scroll_dy_sum,
                        input_dir = input_scroll_dir,
                        cdp_dir = cdp_scroll_dir,
                        dir_match = direction_matches,
                        min_scroll_dy_px,
                        min_confidence = format!("{:.2}", min_confidence),
                        "ignored tiny scroll displacement"
                    );
                }
            }
        }
        let mut trusted_scroll: Option<(i16, f32, &'static str, bool, Option<f32>)> = None;
        let mut detected_scroll_frame: Option<(
            i16,
            f32,
            &'static str,
            bool,
            Option<f32>,
            i32,
            u16,
            u16,
            u16,
        )> = None;

        if let Some(detected_dy) = cdp_scroll_dy_px {
            let cdp_abs = (detected_dy as i32).abs();
            if cdp_abs >= min_scroll_dy_px {
                if pending_scrolls > 0 {
                    if let Some((
                        selected_dy,
                        selected_confidence,
                        source,
                        direction_matches,
                        min_confidence,
                    )) = select_wheel_trusted_scroll(
                        detected_dy,
                        input_scroll_dir,
                        content_scroll,
                    ) {
                        if source == "content" {
                            let dy_gap = ((selected_dy as i32) - (detected_dy as i32)).abs();
                            if dy_gap >= CDP_CONTENT_DY_DIVERGENCE_LOG_PX {
                                trace!(
                                    cdp_dy = detected_dy,
                                    content_dy = selected_dy,
                                    dy_gap,
                                    confidence = format!("{:.2}", selected_confidence),
                                    "wheel scroll dy diverged; preferring content (pixel-aligned)"
                                );
                            }
                        }
                        trusted_scroll = Some((
                            selected_dy,
                            selected_confidence,
                            source,
                            direction_matches,
                            min_confidence,
                        ));
                    } else {
                        let detected_scroll_dir = (detected_dy as i32).signum();
                        let direction_matches = input_scroll_dir == 0
                            || detected_scroll_dir == 0
                            || input_scroll_dir == detected_scroll_dir;
                        trace!(
                            source = "cdp",
                            dy = detected_dy,
                            scrolls = pending_scrolls,
                            input_dy_sum = pending_scroll_dy_sum,
                            input_dir = input_scroll_dir,
                            dir_match = direction_matches,
                            min_scroll_dy_px,
                            "ignored cdp scroll hint with mismatched direction"
                        );
                    }
                } else if let Some((
                    content_dy,
                    content_confidence,
                    content_direction_matches,
                    content_min_confidence,
                )) = content_scroll
                {
                    let cdp_dir = (detected_dy as i32).signum();
                    let content_dir = (content_dy as i32).signum();
                    if cdp_dir == 0 || content_dir == 0 || cdp_dir == content_dir {
                        let dy_gap = ((content_dy as i32) - (detected_dy as i32)).abs();
                        if dy_gap >= CDP_CONTENT_DY_DIVERGENCE_LOG_PX {
                            trace!(
                                cdp_dy = detected_dy,
                                content_dy,
                                dy_gap,
                                confidence = format!("{:.2}", content_confidence),
                                "cdp/content scroll dy diverged; preferring content (pixel-aligned)"
                            );
                        }
                        // Prefer content-based dy: it is derived from the
                        // actual captured frames and therefore pixel-aligned
                        // with the residual comparison.  CDP dy comes from a
                        // different timing domain and may not match the
                        // framebuffer state at capture time.
                        trusted_scroll = Some((
                            content_dy,
                            content_confidence,
                            "content",
                            content_direction_matches,
                            content_min_confidence,
                        ));
                    } else {
                        trace!(
                            cdp_dy = detected_dy,
                            content_dy,
                            confidence = format!("{:.2}", content_confidence),
                            "cdp/content direction mismatch; preferring content"
                        );
                        trusted_scroll = Some((
                            content_dy,
                            content_confidence,
                            "content",
                            content_direction_matches,
                            content_min_confidence,
                        ));
                    }
                } else {
                    // CDP reports a scroll but content-based detection cannot
                    // confirm it in the captured pixels.  This typically means
                    // the frame was captured mid-render — the browser's scrollY
                    // has changed but the framebuffer doesn't yet reflect the
                    // full shift.  Entering scroll mode here would use a delta
                    // that doesn't match the actual pixels, causing the residual
                    // comparison to fail and triggering a costly full repaint.
                    //
                    // Instead, skip scroll mode and emit only the XDamage dirty
                    // tiles as a normal frame.  The next frame capture will
                    // likely see the completed render and content detection will
                    // confirm the scroll then.
                    trace!(
                        source = "cdp-passive-skipped",
                        dy = detected_dy,
                        scrolls = pending_scrolls,
                        cdp_dir = cdp_scroll_dir,
                        "cdp scroll unconfirmed by content detection; skipping scroll mode"
                    );
                }
            } else if pending_scrolls > 0 {
                trace!(
                    source = "cdp",
                    dy = detected_dy,
                    scrolls = pending_scrolls,
                    input_dy_sum = pending_scroll_dy_sum,
                    input_dir = input_scroll_dir,
                    min_scroll_dy_px,
                    "ignored tiny cdp scroll hint"
                );
            }
        }

        if trusted_scroll.is_none() {
            if let Some((dy, confidence, direction_matches, min_confidence)) = content_scroll {
                trusted_scroll =
                    Some((dy, confidence, "content", direction_matches, min_confidence));
            }
        }

        if let Some((detected_dy, confidence, source, direction_matches, min_confidence)) =
            trusted_scroll
        {
            self.content_origin_y += detected_dy as i64;
            // Re-anchor after each trusted scroll so drift correction stays fresh.
            if let Some(cdp_scroll_y_css) = cdp_hint_snapshot.scroll_y {
                let cdp_scroll_y = scale_css_px_to_screen_px(cdp_scroll_y_css, cdp_scale_milli);
                self.cdp_scroll_anchor = Some((cdp_scroll_y, self.content_origin_y));
            }
            self.grid_offset_y = 0;

            // Keep tiles in fixed screen-space. ScrollCopy is only used
            // for whole-tile moves, so row-shift is derived directly from
            // the observed scroll delta rather than a moving tile self.grid.
            let row_shift = if detected_dy as i32 % self.tile_size as i32 == 0 {
                detected_dy as i32 / self.tile_size as i32
            } else {
                0
            };

            // Determine scroll region from CDP viewport hint.
            // If available, limit canvas shift to viewport content area
            // so browser toolbar and scrollbar don't jump.
            let (scroll_region_top, scroll_region_bottom, scroll_region_right) = {
                if let Some(vp) = cdp_hint_snapshot.viewport {
                    (
                        (vp.y as u16).min(self.screen_h),
                        ((vp.y + vp.h) as u16).min(self.screen_h),
                        ((vp.x + vp.w) as u16).min(self.screen_w),
                    )
                } else {
                    (0, self.screen_h, self.screen_w)
                }
            };

            detected_scroll_frame = Some((
                detected_dy,
                confidence,
                source,
                direction_matches,
                min_confidence,
                row_shift,
                scroll_region_top,
                scroll_region_bottom,
                scroll_region_right,
            ));
            strong_scroll_observed = true;
            detected_scroll_dy_px = Some(detected_dy);
        } else {
            // Recalibrate self.content_origin_y against CDP absolute scrollY
            // to eliminate accumulated drift from content-based detection.
            // This ensures scroll-back to a previous position produces
            // identical self.grid_offset_y → identical tile hashes → cache hits.
            //
            // IMPORTANT: only apply when no scroll is detected this frame.
            // If both CDP correction and scroll detection fire in the same
            // frame, the scroll delta is double-counted — CDP correction
            // adds the delta, then scroll detection adds it again — causing
            // self.grid_offset_y to be 2x off and producing ghost/double text.
            if let Some(cdp_scroll_y_css) = cdp_hint_snapshot.scroll_y {
                let cdp_scroll_y = scale_css_px_to_screen_px(cdp_scroll_y_css, cdp_scale_milli);
                if let Some((anchor_scroll_y, anchor_origin)) = self.cdp_scroll_anchor {
                    let expected_origin = anchor_origin + (cdp_scroll_y - anchor_scroll_y);
                    let drift = self.content_origin_y - expected_origin;
                    if drift != 0 {
                        tracing::trace!(
                            drift,
                            self.content_origin_y,
                            expected_origin,
                            cdp_scroll_y,
                            "correcting self.content_origin_y drift from CDP"
                        );
                        self.content_origin_y = expected_origin;
                        self.grid_offset_y = 0;
                    }
                } else {
                    // Establish anchor on first CDP reading
                    self.cdp_scroll_anchor = Some((cdp_scroll_y, self.content_origin_y));
                }
            }
        }
        let cdp_scroll_observed = cdp_scroll_dy_px.is_some();
        self.scroll_active_capture_frames_remaining = next_scroll_active_capture_frames(
            self.scroll_active_capture_frames_remaining,
            self.scroll_active_capture_frames,
            pending_scrolls,
            strong_scroll_observed,
            cdp_scroll_observed,
        );
        if pending_scrolls > 0 || strong_scroll_observed || cdp_scroll_observed {
            // Scroll-like motion always wins over video classification.
            // Keep video suppressed for a short quiet period.
            self.scroll_cooldown_frames = SCROLL_SUPPRESS_VIDEO_FRAMES;
            self.stable_bbox_frames = 0;
            self.prev_video_bbox = None;
            self.scroll_quiet_frames = 0;
        } else if self.scroll_cooldown_frames > 0 {
            self.scroll_cooldown_frames -= 1;
            self.scroll_quiet_frames = self.scroll_quiet_frames.saturating_add(1);
        } else {
            self.scroll_quiet_frames = self.scroll_quiet_frames.saturating_add(1);
        }
        let cdp_video_region_hint_candidate = if self.scroll_cooldown_frames == 0 {
            cdp_video_region_hint_sized
        } else {
            None
        };
        let cdp_click_armed = self.click_armed.update(
            cdp_video_region_hint_candidate,
            self.last_left_click,
            now,
            self.video_click_arm_ms,
            CLICK_LATCH_RESET_FRAMES,
        );
        let cdp_video_region_hint = if cdp_click_armed {
            cdp_video_region_hint_candidate
        } else {
            None
        };
        let cdp_hint_tile_bounds = cdp_video_region_hint
            .map(|region| capture_region_tile_bounds(region, self.tile_size, self.grid.cols, self.grid.rows));

        // ── Per-tile change detection (two-pass) ─────────────
        // Pass 1: hash each tile, extract motion features for changed
        // tiles, identify video candidates, compute candidate bbox.
        let cols = self.grid.cols as usize;
        self.candidate_mask.fill(false);
        self.changed_mask.fill(false);
        self.text_like_mask.fill(false);
        let mut bbox_min_col: u16 = u16::MAX;
        let mut bbox_max_col: u16 = 0;
        let mut bbox_min_row: u16 = u16::MAX;
        let mut bbox_max_row: u16 = 0;
        let mut candidate_count = 0u32;
        let mut strong_candidate_count = 0u32;

        for row in 0..self.grid.rows {
            for col in 0..self.grid.cols {
                let idx = row as usize * cols + col as usize;
                let tx = col as usize * self.tile_size as usize;
                let ty = row as usize * self.tile_size as usize;
                let tw = (self.tile_size as usize).min(self.screen_w as usize - tx);
                let th = (self.tile_size as usize).min(self.screen_h as usize - ty);
                let hash = hash_tile_region(&rgba, stride, tx, ty, tw, th);
                let changed = self.prev_hashes[idx] != 0 && hash != self.prev_hashes[idx];
                self.changed_mask[idx] = changed;

                if changed
                    && self.video_classification_enabled
                    && cdp_video_region_hint.is_some()
                    && self.scroll_cooldown_frames == 0
                {
                    let features = compute_tile_motion_features(
                        &rgba,
                        prev_for_analysis,
                        stride,
                        tx,
                        ty,
                        tw,
                        th,
                    );
                    // Text/UI is typically edge-heavy and lower entropy.
                    // Video/canvas is usually high motion + richer entropy.
                    self.text_like_mask[idx] =
                        features.edge_density > 0.22 && features.entropy_hint < 0.45;
                    let photo_like = is_photo_like_tile(&rgba, stride, tx, ty, tw, th, 16);
                    let video_like = features.change_ratio > 0.23
                        && features.motion_magnitude > 0.045
                        && features.entropy_hint > 0.16
                        && features.edge_density < 0.74
                        && photo_like;
                    let strong_video_like = features.change_ratio > 0.30
                        && features.motion_magnitude > 0.07
                        && features.entropy_hint > 0.20
                        && features.edge_density < 0.70
                        && photo_like;

                    if video_like {
                        self.candidate_mask[idx] = true;
                        candidate_count += 1;
                        if strong_video_like {
                            strong_candidate_count += 1;
                        }
                        bbox_min_col = bbox_min_col.min(col);
                        bbox_max_col = bbox_max_col.max(col);
                        bbox_min_row = bbox_min_row.min(row);
                        bbox_max_row = bbox_max_row.max(row);
                    }
                }
                self.prev_hashes[idx] = hash;
            }
        }

        // Bounding box stability check:
        // Video regions stay roughly stable in position/shape, while
        // scrolling tends to translate the bbox frame to frame.
        let current_bbox = if candidate_count > 0 {
            Some((bbox_min_col, bbox_min_row, bbox_max_col, bbox_max_row))
        } else {
            None
        };

        let bbox_stable = match (current_bbox, self.prev_video_bbox) {
            (Some(cur), Some(prev)) => {
                let iou = bbox_iou(cur, prev);
                let center_shift = bbox_center_shift(cur, prev);
                iou > 0.55 || center_shift <= 1.25
            }
            _ => false,
        };
        if bbox_stable {
            self.stable_bbox_frames = self.stable_bbox_frames.saturating_add(1);
        } else {
            self.stable_bbox_frames = 0;
        }
        self.prev_video_bbox = current_bbox;
        let bbox_tile_area = current_bbox
            .map(|(min_c, min_r, max_c, max_r)| {
                ((max_c - min_c + 1) as u32).saturating_mul((max_r - min_r + 1) as u32)
            })
            .unwrap_or(0);
        let bbox_density = if bbox_tile_area > 0 {
            candidate_count as f32 / bbox_tile_area as f32
        } else {
            0.0
        };
        let (bbox_w_tiles, bbox_h_tiles) = current_bbox
            .map(|(min_c, min_r, max_c, max_r)| (max_c - min_c + 1, max_r - min_r + 1))
            .unwrap_or((0, 0));
        let total_tile_area = (self.grid.cols as u32).saturating_mul(self.grid.rows as u32).max(1);
        let bbox_area_ratio = bbox_tile_area as f32 / total_tile_area as f32;
        let large_stable_region = bbox_w_tiles >= MIN_VIDEO_BBOX_WIDTH_TILES
            && bbox_h_tiles >= MIN_VIDEO_BBOX_HEIGHT_TILES
            && bbox_area_ratio >= MIN_VIDEO_BBOX_AREA_RATIO;
        let region_stable = self.video_classification_enabled
            && cdp_video_region_hint.is_some()
            && self.scroll_cooldown_frames == 0
            && candidate_count >= REGION_MIN_CANDIDATES
            && large_stable_region
            && ((self.stable_bbox_frames >= 4 && bbox_density > 0.34)
                || candidate_count >= REGION_DENSE_CANDIDATES
                || strong_candidate_count >= 8);

        // Pass 2: update per-tile motion scores + hysteresis and classify.
        let mut latched_video_tiles: Vec<tiles::TileCoord> = Vec::new();
        let mut cdp_motion_tiles: u32 = 0;
        for row in 0..self.grid.rows {
            for col in 0..self.grid.cols {
                let idx = row as usize * cols + col as usize;
                let in_cdp_video_hint = cdp_hint_tile_bounds
                    .map(|(min_col, min_row, max_col, max_row)| {
                        col >= min_col && col <= max_col && row >= min_row && row <= max_row
                    })
                    .unwrap_or(false);
                let cdp_motion_candidate =
                    in_cdp_video_hint && self.changed_mask[idx] && !self.text_like_mask[idx];
                if cdp_motion_candidate {
                    cdp_motion_tiles = cdp_motion_tiles.saturating_add(1);
                }
                if region_stable && self.candidate_mask[idx] {
                    self.video_scores[idx] = (self.video_scores[idx] + 4).min(VIDEO_MAX_SCORE);
                    self.non_candidate_streaks[idx] = 0;
                    if self.video_latched[idx] {
                        self.video_hold_frames[idx] = VIDEO_MIN_HOLD_FRAMES;
                    }
                } else if cdp_motion_candidate {
                    self.video_scores[idx] = (self.video_scores[idx] + 2).min(VIDEO_MAX_SCORE);
                    self.non_candidate_streaks[idx] = 0;
                    if self.video_latched[idx] {
                        self.video_hold_frames[idx] = VIDEO_MIN_HOLD_FRAMES;
                    }
                } else if self.changed_mask[idx] {
                    self.video_scores[idx] = (self.video_scores[idx] - 1).max(-VIDEO_MAX_SCORE);
                    self.non_candidate_streaks[idx] = self.non_candidate_streaks[idx].saturating_add(1);
                } else {
                    self.video_scores[idx] = (self.video_scores[idx] - 1).max(-VIDEO_MAX_SCORE);
                    self.non_candidate_streaks[idx] = self.non_candidate_streaks[idx].saturating_add(1);
                }

                if !(region_stable && self.candidate_mask[idx] || cdp_motion_candidate)
                    && self.video_hold_frames[idx] > 0
                {
                    self.video_hold_frames[idx] -= 1;
                }

                if !self.video_classification_enabled
                    || cdp_video_region_hint.is_none()
                    || self.scroll_cooldown_frames > 0
                {
                    self.video_latched[idx] = false;
                    self.video_hold_frames[idx] = 0;
                    self.video_scores[idx] = self.video_scores[idx].min(0);
                } else if self.video_latched[idx] {
                    if self.video_hold_frames[idx] == 0
                        && (self.video_scores[idx] <= VIDEO_EXIT_SCORE
                            || self.non_candidate_streaks[idx] >= VIDEO_DECAY_STREAK)
                    {
                        self.video_latched[idx] = false;
                        self.video_hold_frames[idx] = 0;
                    }
                } else if (region_stable && self.video_scores[idx] >= VIDEO_ENTER_SCORE)
                    || (cdp_motion_candidate && self.video_scores[idx] >= (VIDEO_ENTER_SCORE - 1))
                {
                    self.video_latched[idx] = true;
                    self.video_hold_frames[idx] = VIDEO_MIN_HOLD_FRAMES;
                }

                if let Some(tile) = self.grid.get_mut(tiles::TileCoord::new(col, row)) {
                    tile.dirty = true;
                    tile.classification = if self.video_latched[idx] {
                        tiles::TileClass::VideoMotion
                    } else if self.changed_mask[idx] && self.text_like_mask[idx] {
                        tiles::TileClass::TextScroll
                    } else {
                        tiles::TileClass::Static
                    };
                }
                if self.video_latched[idx] {
                    latched_video_tiles.push(tiles::TileCoord::new(col, row));
                }
            }
        }
        // Emit tiles for all positions, including extra bottom row when
        // self.grid offset is active (partial tile at bottom edge).
        let emit_rows = if self.grid_offset_y > 0 {
            self.grid.rows + 1
        } else {
            self.grid.rows
        };
        let full_emit_coords: Vec<tiles::TileCoord> = (0..emit_rows)
            .flat_map(|r| (0..self.grid.cols).map(move |c| tiles::TileCoord::new(c, r)))
            .collect();
        // Narrow dirty set to tiles overlapping XDamage bounding box.
        // Tiles outside the self.damage region are guaranteed unchanged by the
        // X server, so we skip their extraction + hashing entirely.
        // During scroll or force-refresh we fall back to all tiles.
        let mut all_dirty: Vec<tiles::TileCoord> = if !force_refresh {
            if let Some(ref dt) = self.damage {
                if let Some((dx, dy, dw, dh)) = dt.damage_bounding_box() {
                    let ts = self.tile_size as u16;
                    let dx2 = dx.saturating_add(dw);
                    let dy2 = dy.saturating_add(dh);
                    full_emit_coords
                        .iter()
                        .copied()
                        .filter(|coord| {
                            let tx = coord.col * ts;
                            let ty = coord.row * ts;
                            let tx2 = tx.saturating_add(ts);
                            let ty2 = ty.saturating_add(ts);
                            // AABB overlap test
                            tx < dx2 && tx2 > dx && ty < dy2 && ty2 > dy
                        })
                        .collect()
                } else {
                    full_emit_coords.clone()
                }
            } else {
                full_emit_coords.clone()
            }
        } else {
            full_emit_coords.clone()
        };
        let mut scroll_residual_ratio: Option<f32> = None;
        let mut scroll_residual_fallback_full = false;
        let mut scroll_residual_tiles_frame: Option<usize> = None;
        let mut scroll_potential_tiles_frame: Option<usize> = None;
        let mut scroll_saved_tiles_frame: Option<usize> = None;
        let mut scroll_saved_ratio_frame: Option<f32> = None;
        let mut scroll_emit_ratio_frame: Option<f32> = None;
        let mut scroll_thin_mode_frame = false;
        let mut scroll_thin_repair_frame = false;
        if let (Some(scroll_dy), Some(prev)) = (detected_scroll_dy_px, prev_for_analysis) {
            let scroll_row_shift = detected_scroll_frame
                .map(|(_, _, _, _, _, row_shift, _, _, _)| row_shift)
                .unwrap_or(0);
            // Get scroll region bounds for partitioning tiles into
            // content (scrollable) and chrome (static).  Use freshly
            // detected values when available, fall back to cached.
            let srt_for_split = detected_scroll_frame
                .map(|(_, _, _, _, _, _, srt, _, _)| srt)
                .unwrap_or(self.last_scroll_region_top);
            let srb_for_split = detected_scroll_frame
                .map(|(_, _, _, _, _, _, _, srb, _)| srb)
                .unwrap_or(self.screen_h);
            let srr_for_split = detected_scroll_frame
                .map(|(_, _, _, _, _, _, _, _, srr)| srr)
                .unwrap_or(self.last_scroll_region_right);
            let ts = self.tile_size as u16;

            // Partition: content tiles are below chrome header and left
            // of scrollbar; everything else is chrome / static.
            // Chrome exists regardless of sub-tile offset; partition
            // whenever we have a detected scroll region.
            let have_split = has_scroll_region_split(
                srt_for_split,
                srb_for_split,
                srr_for_split,
                self.screen_h,
                self.screen_w,
            );
            let (content_emit_coords, chrome_emit_coords): (
                Vec<tiles::TileCoord>,
                Vec<tiles::TileCoord>,
            ) = if have_split {
                full_emit_coords.iter().partition(|coord| {
                    // Only tiles fully inside the scrollable viewport are
                    // eligible for ScrollCopy reuse. Boundary tiles that
                    // overlap the header, bottom seam, or scrollbar remain
                    // raw/static so the host never assumes partially moved
                    // tiles are already correct on the client.
                    is_content_tile_in_scroll_region(
                        **coord,
                        ts,
                        srt_for_split,
                        srb_for_split,
                        srr_for_split,
                    )
                })
            } else {
                (full_emit_coords.clone(), Vec::new())
            };

            let residual_coords = content_emit_coords.clone();
            let residual = build_scroll_residual_emit_coords(
                &rgba,
                prev,
                stride,
                &self.grid,
                self.grid_offset_y,
                scroll_dy,
                &residual_coords,
            );

            let potential_tiles = residual_coords.len();
            let residual_tiles = residual.len();
            let residual_ratio = if potential_tiles == 0 {
                1.0
            } else {
                residual_tiles as f32 / potential_tiles as f32
            };
            self.scroll_residual_batches_total = self.scroll_residual_batches_total.saturating_add(1);
            self.scroll_potential_tiles_total =
                self.scroll_potential_tiles_total.saturating_add(potential_tiles as u64);
            self.scroll_residual_tiles_total =
                self.scroll_residual_tiles_total.saturating_add(residual_tiles as u64);
            scroll_residual_tiles_frame = Some(residual_tiles);
            scroll_potential_tiles_frame = Some(potential_tiles);
            scroll_residual_ratio = Some(residual_ratio);

            // Fallback decision: only triggers if the residual ratio
            // for INTERIOR content tiles (excluding newly exposed edges)
            // exceeds the threshold, indicating scroll detection inaccuracy.
            let exposed_tiles = build_scroll_exposed_strip_emit_coords(
                &self.grid,
                self.grid_offset_y,
                scroll_dy,
                &residual_coords,
            );
            let interior_residual = residual
                .iter()
                .filter(|c| !exposed_tiles.contains(c))
                .count();
            let interior_total = potential_tiles.saturating_sub(exposed_tiles.len());
            let interior_ratio = if interior_total == 0 {
                0.0
            } else {
                interior_residual as f32 / interior_total as f32
            };
            let quantized_scroll_copy =
                can_emit_scroll_copy(scroll_dy, self.scroll_copy_quantum_px, self.tile_size);
            let saved_tiles = potential_tiles.saturating_sub(residual_tiles);
            let defer_scroll_repair = should_defer_scroll_repair(
                quantized_scroll_copy,
                interior_ratio,
                saved_tiles,
                potential_tiles,
                scroll_row_shift,
            );
            if !quantized_scroll_copy {
                trace!(
                    dy = scroll_dy,
                    scroll_copy_quantum_px = self.scroll_copy_quantum_px,
                    "scroll copy suppressed for non-quantized delta"
                );
                scroll_residual_fallback_full = true;
                self.scroll_residual_fallback_full_total =
                    self.scroll_residual_fallback_full_total.saturating_add(1);
                scroll_saved_tiles_frame = Some(0);
                scroll_saved_ratio_frame = Some(0.0);
                scroll_emit_ratio_frame = Some(1.0);
                self.scroll_thin_mode_active = false;
                self.scroll_residual_was_active = false;
                // Restore full dirty set — XDamage narrowing may have
                // excluded tiles that need a full repaint after scroll.
                all_dirty = full_emit_coords.clone();
            } else if interior_ratio > SCROLL_RESIDUAL_FULL_REPAINT_RATIO
                && !defer_scroll_repair
            {
                trace!(
                    dy = scroll_dy,
                    row_shift = scroll_row_shift,
                    interior_ratio = format!("{:.2}", interior_ratio),
                    saved_tiles,
                    potential_tiles,
                    "scroll copy suppressed by residual full repaint threshold"
                );
                scroll_residual_fallback_full = true;
                self.scroll_residual_fallback_full_total =
                    self.scroll_residual_fallback_full_total.saturating_add(1);
                scroll_saved_tiles_frame = Some(0);
                scroll_saved_ratio_frame = Some(0.0);
                scroll_emit_ratio_frame = Some(1.0);
                self.scroll_thin_mode_active = false;
                self.scroll_residual_was_active = false;
                // Restore full dirty set — XDamage narrowing may have
                // excluded tiles that need a full repaint after scroll.
                all_dirty = full_emit_coords.clone();
            } else {
                // Dirty set = residual content + chrome.
                all_dirty = residual;
                all_dirty.extend(chrome_emit_coords.iter().copied());

                // Force content tiles overlapping the client-side exposed
                // strip into the dirty set.  The residual analysis compares
                // pixels (white == white on a uniform page) so these tiles
                // pass the check, but ScrollCopy cleared their canvas region.
                // Without this, they'd be skipped → black band.
                {
                    let (exp_start, exp_end) = if scroll_dy > 0 {
                        let start =
                            (srb_for_split as i32 - scroll_dy as i32).max(srt_for_split as i32);
                        (start, srb_for_split as i32)
                    } else {
                        let end = (srt_for_split as i32 + (-scroll_dy) as i32)
                            .min(srb_for_split as i32);
                        (srt_for_split as i32, end)
                    };
                    for &coord in &content_emit_coords {
                        if all_dirty.contains(&coord) {
                            continue;
                        }
                        let rect = offset_tile_rect_for_emit(coord, &self.grid, self.grid_offset_y);
                        if rect.w == 0 || rect.h == 0 {
                            continue;
                        }
                        let tile_top = rect.y as i32;
                        let tile_bot = tile_top + rect.h as i32;
                        if tile_bot > exp_start && tile_top < exp_end {
                            all_dirty.push(coord);
                        }
                    }
                }
                self.scroll_saved_tiles_total =
                    self.scroll_saved_tiles_total.saturating_add(saved_tiles as u64);
                scroll_saved_tiles_frame = Some(saved_tiles);
                if potential_tiles > 0 {
                    scroll_saved_ratio_frame =
                        Some(saved_tiles as f32 / potential_tiles as f32);
                    scroll_emit_ratio_frame =
                        Some(residual_tiles as f32 / potential_tiles as f32);
                } else {
                    scroll_saved_ratio_frame = Some(0.0);
                    scroll_emit_ratio_frame = Some(1.0);
                }
                self.scroll_residual_was_active = saved_tiles > 0;
                if defer_scroll_repair {
                    trace!(
                        dy = scroll_dy,
                        row_shift = scroll_row_shift,
                        interior_ratio = format!("{:.2}", interior_ratio),
                        saved_tiles,
                        potential_tiles,
                        "scroll copy accepted with deferred repair"
                    );
                }

                // If sub-tile residual explodes, prioritize exposed strip
                // during active scrolling and defer one full repair frame.
                let sub_tile_scroll = (scroll_dy.unsigned_abs() as u16) < self.tile_size;
                let residual_tiles_min_for_thin = (self.grid.cols as usize * 2).max(12);
                let residual_large_for_sub_tile = residual_ratio
                    >= SCROLL_THIN_MODE_RESIDUAL_RATIO
                    || residual_tiles >= residual_tiles_min_for_thin;
                if sub_tile_scroll && residual_large_for_sub_tile {
                    let strip_dirty = build_scroll_exposed_strip_emit_coords(
                        &self.grid,
                        self.grid_offset_y,
                        scroll_dy,
                        &residual_coords,
                    );
                    if self.scroll_thin_mode_enabled && !strip_dirty.is_empty() {
                        // Keep chrome tiles in dirty set for static emit.
                        all_dirty = strip_dirty;
                        all_dirty.extend(chrome_emit_coords.iter().copied());
                        self.scroll_thin_mode_active = true;
                        scroll_thin_mode_frame = true;
                        self.scroll_thin_batches_total = self.scroll_thin_batches_total.saturating_add(1);
                    } else {
                        self.scroll_thin_mode_active = false;
                    }
                } else {
                    self.scroll_thin_mode_active = false;
                }
            }
        } else if (self.scroll_thin_mode_active || self.scroll_residual_was_active)
            && self.scroll_quiet_frames >= SCROLL_THIN_REPAIR_QUIET_FRAMES
        {
            // Reconcile: after scroll quiets, force a full tile emit to
            // correct any accumulated errors from ScrollCopy + residual skipping.
            // This ensures tiles skipped by residual analysis (whose last_hashes
            // are stale) get properly re-evaluated and updated.
            all_dirty = full_emit_coords.clone();
            self.scroll_thin_mode_active = false;
            self.scroll_residual_was_active = false;
            scroll_thin_repair_frame = true;
            self.scroll_thin_repairs_total = self.scroll_thin_repairs_total.saturating_add(1);
        }

        let emit_scroll_copy =
            should_emit_scroll_copy(scroll_residual_fallback_full, scroll_saved_tiles_frame);
        if let Some((
            detected_dy,
            confidence,
            source,
            direction_matches,
            min_confidence,
            row_shift,
            scroll_region_top,
            scroll_region_bottom,
            scroll_region_right,
        )) = detected_scroll_frame
        {
            if emit_scroll_copy {
                // Only shift hashes when ScrollCopy is actually sent —
                // keeps last_hashes consistent with the client canvas.
                if row_shift != 0 {
                    self.emitter.shift_hashes(row_shift, self.grid.rows);
                }
                // Always zero exposed strip when sending ScrollCopy,
                // even for sub-tile scrolls (row_shift == 0).  The client
                // keeps that strip stale until repair tiles arrive; if we
                // don't zero the corresponding hashes, L1 skip can keep
                // the stale strip visible indefinitely.
                self.emitter.zero_exposed_strip(
                    detected_dy,
                    scroll_region_top,
                    scroll_region_bottom,
                    self.tile_size,
                    self.grid_offset_y,
                );
                let scroll_frame = bpane_protocol::TileMessage::ScrollCopy {
                    dx: 0,
                    dy: detected_dy,
                    region_top: scroll_region_top,
                    region_bottom: scroll_region_bottom,
                    region_right: scroll_region_right,
                }
                .to_frame();
                if self.tile_tx.blocking_send(scroll_frame).is_err() {
                    return;
                }
            }

            let offset_frame = bpane_protocol::TileMessage::GridOffset {
                offset_x: 0,
                offset_y: self.grid_offset_y as i16,
            }
            .to_frame();
            if self.tile_tx.blocking_send(offset_frame).is_err() {
                return;
            }

            tracing::debug!(
                source,
                dy = detected_dy,
                confidence = format!("{:.2}", confidence),
                offset_y = self.grid_offset_y,
                row_shift = row_shift,
                scroll_copy = emit_scroll_copy,
                scrolls = pending_scrolls,
                input_dy_sum = pending_scroll_dy_sum,
                input_dir = input_scroll_dir,
                cdp_dir = cdp_scroll_dir,
                dir_match = direction_matches,
                min_scroll_dy_px,
                min_confidence = min_confidence.map(|c| format!("{:.2}", c)),
                "scroll detected"
            );
        }

        // Export cumulative host-side residual telemetry to client for
        // direct Scroll Health reporting in test dashboards.
        let to_u32_sat = |v: u64| -> u32 { v.min(u32::MAX as u64) as u32 };
        let scroll_stats_frame = bpane_protocol::TileMessage::ScrollStats {
            scroll_batches_total: to_u32_sat(self.scroll_residual_batches_total),
            scroll_full_fallbacks_total: to_u32_sat(self.scroll_residual_fallback_full_total),
            scroll_potential_tiles_total: to_u32_sat(self.scroll_potential_tiles_total),
            scroll_saved_tiles_total: to_u32_sat(self.scroll_saved_tiles_total),
        }
        .to_frame();
        if self.tile_tx.blocking_send(scroll_stats_frame).is_err() {
            return;
        }

        // Split dirty tiles into static (browser chrome) and content (scrolling
        // viewport). Static tiles are emitted at raw framebuffer positions
        // with a separate hash table that is never shifted, so browser
        // header/scrollbar tiles achieve L1 cache hits across scroll frames.
        if let Some((_, _, _, _, _, _, srt, srb, srr)) = detected_scroll_frame {
            self.last_scroll_region_top = srt;
            self.last_scroll_region_bottom = srb;
            self.last_scroll_region_right = srr;
        }
        if key_input_qoi_boost {
            if let Some(bounds) = editable_qoi_tile_bounds {
                extend_dirty_with_tile_bounds(&mut all_dirty, bounds);
            }
        }
        let use_static_split = has_scroll_region_split(
            self.last_scroll_region_top,
            self.last_scroll_region_bottom,
            self.last_scroll_region_right,
            self.screen_h,
            self.screen_w,
        );
        let (content_dirty, static_dirty): (Vec<tiles::TileCoord>, Vec<tiles::TileCoord>) =
            if use_static_split {
                let ts = self.tile_size as u16;
                let srt = self.last_scroll_region_top;
                let srb = self.last_scroll_region_bottom;
                let srr = self.last_scroll_region_right;
                all_dirty.iter().partition(|coord| {
                    // Must stay in sync with the residual-analysis
                    // partitioning above.
                    is_content_tile_in_scroll_region(**coord, ts, srt, srb, srr)
                })
            } else {
                (all_dirty, Vec::new())
            };

        let result = self.emitter.emit_frame(
            &rgba,
            stride,
            &content_dirty,
            &self.grid,
            self.grid_offset_y,
            editable_qoi_tile_bounds,
        );

        // Emit static (chrome) tiles separately with offset_y=0.
        let static_frames = if !static_dirty.is_empty() {
            let draw_mode_off = bpane_protocol::TileMessage::TileDrawMode {
                apply_offset: false,
            }
            .to_frame();
            let ts = self.tile_size as u16;
            let boundary_col = if use_static_split
                && self.last_scroll_region_right < self.screen_w
                && self.last_scroll_region_right % ts != 0
            {
                Some(self.last_scroll_region_right / ts)
            } else {
                None
            };
            let boundary_top_row = if use_static_split
                && self.last_scroll_region_top > 0
                && self.last_scroll_region_top % ts != 0
            {
                Some(self.last_scroll_region_top / ts)
            } else {
                None
            };
            let boundary_bottom_row = if use_static_split
                && self.last_scroll_region_bottom < self.screen_h
                && self.last_scroll_region_bottom % ts != 0
            {
                Some(self.last_scroll_region_bottom / ts)
            } else {
                None
            };
            let tiles = self.emitter.emit_static_tiles(
                &rgba,
                stride,
                &static_dirty,
                &self.grid,
                boundary_col,
                boundary_top_row,
                boundary_bottom_row,
                editable_qoi_tile_bounds,
            );
            let draw_mode_on =
                bpane_protocol::TileMessage::TileDrawMode { apply_offset: true }.to_frame();
            let mut out = Vec::with_capacity(tiles.len() + 2);
            out.push(draw_mode_off);
            out.extend(tiles);
            out.push(draw_mode_on);
            out
        } else {
            Vec::new()
        };
        self.grid.clear_dirty();

        let static_tile_count = static_frames.len().saturating_sub(2); // minus DrawMode on/off
        let static_bytes: usize = static_frames.iter().map(|f| f.payload.len()).sum();

        let s = &result.stats;
        if s.fills > 0 || s.qoi_tiles > 0 || s.cache_hits > 0 || scroll_residual_ratio.is_some()
        {
            let scroll_saved_rate_total = if self.scroll_potential_tiles_total > 0 {
                Some(self.scroll_saved_tiles_total as f32 / self.scroll_potential_tiles_total as f32)
            } else {
                None
            };
            tracing::trace!(
                skipped = s.skipped,
                fills = s.fills,
                cache_hits = s.cache_hits,
                qoi = s.qoi_tiles,
                video_changed = s.video_tiles,
                self.video_latched = latched_video_tiles.len(),
                cdp_motion_tiles,
                cdp_hint_raw = cdp_hint_region_raw.is_some(),
                cdp_video_hint = cdp_video_region_hint.is_some(),
                editable_qoi = editable_qoi_tile_bounds.is_some(),
                key_input_qoi_boost,
                self.click_armed = cdp_click_armed,
                scroll_suppress = self.scroll_cooldown_frames,
                scroll_residual_ratio = scroll_residual_ratio.map(|r| format!("{:.2}", r)),
                scroll_residual_full = scroll_residual_fallback_full,
                scroll_residual_tiles = scroll_residual_tiles_frame,
                scroll_potential_tiles = scroll_potential_tiles_frame,
                scroll_saved_tiles = scroll_saved_tiles_frame,
                scroll_saved_ratio = scroll_saved_ratio_frame.map(|r| format!("{:.2}", r)),
                scroll_emit_ratio = scroll_emit_ratio_frame.map(|r| format!("{:.2}", r)),
                scroll_batches_total = self.scroll_residual_batches_total,
                scroll_full_fallbacks_total = self.scroll_residual_fallback_full_total,
                self.scroll_potential_tiles_total,
                self.scroll_residual_tiles_total,
                self.scroll_saved_tiles_total,
                scroll_saved_rate_total = scroll_saved_rate_total.map(|r| format!("{:.2}", r)),
                scroll_thin_mode = scroll_thin_mode_frame,
                scroll_thin_repair = scroll_thin_repair_frame,
                scroll_thin_active = self.scroll_thin_mode_active,
                self.scroll_thin_batches_total,
                self.scroll_thin_repairs_total,
                qoi_kb = s.qoi_bytes / 1024,
                static_tiles = static_tile_count,
                static_bytes,
                content_dirty = content_dirty.len(),
                static_dirty_count = static_dirty.len(),
                use_static_split,
                "tile frame"
            );
        }

        // H.264 region management
        let cdp_has_video = cdp_video_region_hint.is_some() && self.scroll_cooldown_frames == 0;
        let _h264_update = self.update_h264_region(
            cdp_video_region_hint,
            cdp_has_video,
            cdp_motion_tiles,
            now,
            REGION_RECONFIG_STABLE_FRAMES,
            REGION_RECONFIG_MIN_INTERVAL_MS,
            MIN_CHANGED_VIDEO_TILES_FOR_H264,
            H264_MIN_ON_DURATION_MS,
        );

        // Send all tile data (content + static) BEFORE BatchEnd so the
        // client processes everything in a single batch.  Previously,
        // static tiles were sent after BatchEnd, causing them to be
        // deferred to the next batch — leaving the buffer row (header/
        // content seam) black for one frame during scroll.
        //
        // Order: content tiles (sans BatchEnd) → static tiles → BatchEnd.
        let mut content_frames = result.tile_frames;
        let batch_end_frame = content_frames.pop(); // always BatchEnd
        for frame in content_frames {
            if self.tile_tx.blocking_send(frame).is_err() {
                return;
            }
        }
        for frame in static_frames {
            if self.tile_tx.blocking_send(frame).is_err() {
                return;
            }
        }
        if let Some(be) = batch_end_frame {
            if self.tile_tx.blocking_send(be).is_err() {
                return;
            }
        }

        if let Some(dt) = self.damage.as_mut() {
            dt.reset();
        }

        // Keep current frame as previous without cloning.
        self.prev_frame = Some(rgba);
    }

    }
}
