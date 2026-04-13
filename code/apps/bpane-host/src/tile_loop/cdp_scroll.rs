//! CDP hint processing and scroll detection for the current frame.
//!
//! Reads the browser CDP hint state, processes editable/video regions,
//! detects scroll displacement via content-based column matching, and
//! resolves trusted scroll from CDP + content sources.

use tracing::trace;

use crate::capture::ffmpeg::CaptureRegion;
use crate::cdp_video;
use crate::region::{
    capture_region_tile_bounds, clamp_region_to_screen, expand_tile_bounds,
    region_meets_editable_minimum, region_meets_video_minimum,
    scale_css_px_to_screen_px,
};
use crate::scroll::{
    can_emit_scroll_copy, content_scroll_search_limit_px, detect_column_scroll,
    is_scroll_delta_quantized, next_scroll_active_capture_frames, select_wheel_trusted_scroll,
};
use crate::tiles;

use super::frame_types::{CdpScrollResult, DetectedScrollFrame};

impl super::TileCaptureThread {
    /// Process CDP hints, detect scroll, and resolve trusted scroll source.
    ///
    /// This is the first phase of per-frame processing after capture.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn process_cdp_and_scroll(
        &mut self,
        rgba: &[u8],
        stride: usize,
        now: std::time::Instant,
        force_refresh: bool,
        editable_hint_hold_ms: u64,
        key_input_qoi_boost_ms: u64,
        editable_qoi_tile_margin: u16,
        max_cdp_scroll_dy_px: i64,
        cdp_content_dy_divergence_log_px: i32,
        min_scroll_dy_px: i32,
        input_min_scroll_dy_px: i32,
        input_scroll_min_confidence: f32,
        no_input_scroll_min_confidence: f32,
        scroll_suppress_video_frames: u8,
        scroll_copy_quantum_px: u16,
    ) -> CdpScrollResult {
        const CLICK_LATCH_RESET_FRAMES: u8 = 20;

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
        self.editable_hint.update(cdp_editable_region_hint, now, editable_hint_hold_ms);
        let key_input_qoi_boost = self.editable_hint.key_input_qoi_boost(now, key_input_qoi_boost_ms);
        let editable_qoi_region = self.editable_hint.qoi_region(
            cdp_editable_region_hint,
            now,
            editable_hint_hold_ms,
            key_input_qoi_boost_ms,
        );
        let editable_qoi_tile_bounds = editable_qoi_region.map(|region| {
            expand_tile_bounds(
                capture_region_tile_bounds(region, self.tile_size, self.grid.cols, self.grid.rows),
                editable_qoi_tile_margin,
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
                    let clamped = dy.clamp(-max_cdp_scroll_dy_px, max_cdp_scroll_dy_px) as i16;
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
            .clamp(-max_cdp_scroll_dy_px, max_cdp_scroll_dy_px)
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
            input_min_scroll_dy_px
        } else {
            min_scroll_dy_px
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
                    input_scroll_min_confidence
                } else {
                    no_input_scroll_min_confidence
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
        let mut detected_scroll_frame: Option<DetectedScrollFrame> = None;

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
                            if dy_gap >= cdp_content_dy_divergence_log_px {
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
                        if dy_gap >= cdp_content_dy_divergence_log_px {
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

            detected_scroll_frame = Some(DetectedScrollFrame {
                dy: detected_dy,
                confidence,
                source,
                direction_matches,
                min_confidence,
                row_shift,
                region_top: scroll_region_top,
                region_bottom: scroll_region_bottom,
                region_right: scroll_region_right,
            });
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
            self.scroll_cooldown_frames = scroll_suppress_video_frames;
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


        CdpScrollResult {
            cdp_video_region_hint,
            cdp_hint_tile_bounds,
            editable_qoi_tile_bounds,
            key_input_qoi_boost,
            pending_scrolls,
            pending_scroll_dy_sum,
            input_scroll_dir,
            cdp_scroll_dy_px,
            strong_scroll_observed,
            detected_scroll_frame,
        }
    }
}
