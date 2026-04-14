//! Scroll resolution: content-based scroll detection, trust arbitration
//! between CDP and content sources, drift correction, cooldown management,
//! and active video hint resolution.

use tracing::trace;

use crate::capture::ffmpeg::CaptureRegion;
use crate::region::{capture_region_tile_bounds, scale_css_px_to_screen_px};
use crate::scroll::{
    content_scroll_search_limit_px, detect_column_scroll, next_scroll_active_capture_frames,
};

use super::frame_types::{CdpScrollResult, DetectedScrollFrame};
use super::scroll_trust::arbitrate_scroll_trust;

impl super::TileCaptureThread {
    /// Content-based scroll detection, trust arbitration between CDP and
    /// content sources, drift correction, cooldown management, and
    /// active video hint resolution.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve_scroll(
        &mut self,
        rgba: &[u8],
        stride: usize,
        _now: std::time::Instant,
        cdp_hint_snapshot: &crate::cdp_video::PageHintState,
        cdp_video_region_hint_sized: Option<CaptureRegion>,
        pending_scrolls: i32,
        pending_scroll_dy_sum: i32,
        cdp_scroll_dy_px: Option<i16>,
        cdp_content_dy_divergence_log_px: i32,
        min_scroll_dy_px: i32,
        input_min_scroll_dy_px: i32,
        input_scroll_min_confidence: f32,
        no_input_scroll_min_confidence: f32,
        scroll_suppress_video_frames: u8,
    ) -> CdpScrollResult {
        let prev_for_analysis = self.prev_frame.as_deref();

        // Content-based scroll detection via vertical column matching.
        let mut strong_scroll_observed = false;
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
                let min_confidence = if hint_scroll_dir != 0 && direction_matches {
                    input_scroll_min_confidence
                } else {
                    no_input_scroll_min_confidence
                };
                let trusted = detected_dy.abs() >= min_scroll_dy_px && confidence >= min_confidence;
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

        // Trust arbitration between CDP and content sources.
        let trusted_scroll = arbitrate_scroll_trust(
            cdp_scroll_dy_px,
            content_scroll,
            pending_scrolls,
            pending_scroll_dy_sum,
            input_scroll_dir,
            cdp_scroll_dir,
            min_scroll_dy_px,
            cdp_content_dy_divergence_log_px,
        );

        let cdp_scale_milli = cdp_hint_snapshot.device_scale_factor_milli.max(1);
        let mut detected_scroll_frame: Option<DetectedScrollFrame> = None;

        if let Some((detected_dy, confidence, source, direction_matches, min_confidence)) =
            trusted_scroll
        {
            self.content_origin_y += detected_dy as i64;
            if let Some(cdp_scroll_y_css) = cdp_hint_snapshot.scroll_y {
                let cdp_scroll_y = scale_css_px_to_screen_px(cdp_scroll_y_css, cdp_scale_milli);
                self.cdp_scroll_anchor = Some((cdp_scroll_y, self.content_origin_y));
            }
            self.grid_offset_y = 0;
            let row_shift = if detected_dy as i32 % self.tile_size as i32 == 0 {
                detected_dy as i32 / self.tile_size as i32
            } else {
                0
            };
            let (scroll_region_top, scroll_region_bottom, scroll_region_right) =
                if let Some(vp) = cdp_hint_snapshot.viewport {
                    (
                        (vp.y as u16).min(self.screen_h),
                        ((vp.y + vp.h) as u16).min(self.screen_h),
                        ((vp.x + vp.w) as u16).min(self.screen_w),
                    )
                } else {
                    (0, self.screen_h, self.screen_w)
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
        } else {
            // Drift correction against CDP absolute scrollY.
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
        let cdp_video_region_hint = if self.scroll_cooldown_frames == 0 {
            cdp_video_region_hint_sized
        } else {
            None
        };
        let cdp_hint_tile_bounds = cdp_video_region_hint.map(|region| {
            capture_region_tile_bounds(region, self.tile_size, self.grid.cols, self.grid.rows)
        });

        CdpScrollResult {
            cdp_video_region_hint,
            cdp_hint_tile_bounds,
            editable_qoi_tile_bounds: None,
            key_input_qoi_boost: false,
            pending_scrolls,
            pending_scroll_dy_sum,
            input_scroll_dir,
            cdp_scroll_dy_px,
            detected_scroll_frame,
        }
    }
}
