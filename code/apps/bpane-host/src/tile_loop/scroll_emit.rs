//! Scroll residual analysis, thin-mode management, dirty-set
//! computation, and scroll frame emission orchestrator.
//!
//! Delegates to `dirty_set`, `scroll_residual`, and `scroll_send`
//! for the three phases of scroll emission processing.

use super::frame_types::{DetectedScrollFrame, ScrollEmitResult};

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
        let (full_emit_coords, all_dirty) = self.build_dirty_set(force_refresh);

        // Phase 2: Analyze scroll residual
        let residual = self.analyze_scroll_residual(
            rgba,
            stride,
            &full_emit_coords,
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
