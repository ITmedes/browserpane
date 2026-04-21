//! Scroll residual analysis, thin-mode management, dirty-set
//! computation, and scroll frame emission orchestrator.
//!
//! Delegates to `dirty_set`, `scroll_residual`, and `scroll_send`
//! for the three phases of scroll emission processing.

use super::frame_types::{DetectedScrollFrame, ScrollEmitResult};

fn should_build_dirty_set_for_frame(
    has_prev_frame: bool,
    detected_scroll_frame: &Option<DetectedScrollFrame>,
) -> bool {
    !has_prev_frame || detected_scroll_frame.is_none()
}

impl super::TileCaptureThread {
    /// Compute scroll residual, manage thin mode, build the dirty tile set,
    /// and emit scroll frames.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compute_scroll_and_dirty(
        &mut self,
        rgba: &[u8],
        stride: usize,
        force_refresh: bool,
        detected_scroll_frame: Option<DetectedScrollFrame>,
        pending_scrolls: i32,
        pending_scroll_dy_sum: i32,
        input_scroll_dir: i32,
        cdp_scroll_dy_px: Option<i16>,
        scroll_residual_full_repaint_ratio: f32,
        scroll_thin_mode_residual_ratio: f32,
        scroll_thin_repair_quiet_frames: u8,
    ) -> ScrollEmitResult {
        // Phase 1: Build dirty set from XDamage
        let all_dirty = if should_build_dirty_set_for_frame(
            self.prev_frame.is_some(),
            &detected_scroll_frame,
        ) {
            self.build_dirty_set(force_refresh)
        } else {
            Vec::new()
        };

        // Phase 2: Analyze scroll residual
        let residual = self.analyze_scroll_residual(
            rgba,
            stride,
            all_dirty,
            &detected_scroll_frame,
            scroll_residual_full_repaint_ratio,
            scroll_thin_mode_residual_ratio,
            scroll_thin_repair_quiet_frames,
        );

        // Phase 3: Send scroll frames
        self.send_scroll_frames(
            detected_scroll_frame,
            pending_scrolls,
            pending_scroll_dy_sum,
            input_scroll_dir,
            cdp_scroll_dy_px,
            residual,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::{should_build_dirty_set_for_frame, DetectedScrollFrame};

    fn detected_scroll_frame() -> DetectedScrollFrame {
        DetectedScrollFrame {
            dy: 64,
            confidence: 0.95,
            source: "content",
            direction_matches: true,
            min_confidence: Some(0.80),
            row_shift: 1,
            region_top: 0,
            region_bottom: 768,
            region_right: 1280,
        }
    }

    #[test]
    fn builds_dirty_set_when_scroll_analysis_cannot_run() {
        assert!(should_build_dirty_set_for_frame(false, &Some(detected_scroll_frame())));
        assert!(should_build_dirty_set_for_frame(true, &None));
    }

    #[test]
    fn skips_dirty_set_when_trusted_scroll_can_reuse_full_emit_coords() {
        assert!(!should_build_dirty_set_for_frame(
            true,
            &Some(detected_scroll_frame())
        ));
    }
}
