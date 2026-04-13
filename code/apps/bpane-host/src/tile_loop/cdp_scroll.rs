//! CDP hint processing and scroll detection orchestrator.
//!
//! Delegates to `cdp_hints`, `cdp_scroll_track`, and `scroll_resolve`
//! for the three phases of per-frame CDP + scroll processing.

use super::frame_types::CdpScrollResult;

impl super::TileCaptureThread {
    /// Process CDP hints, detect scroll, and resolve trusted scroll source.
    ///
    /// This is the first phase of per-frame processing after capture.
    /// Orchestrates three sub-phases:
    /// 1. `snapshot_cdp_hints` - drain events, read hint state
    /// 2. `track_cdp_scroll` - extract CDP scroll displacement
    /// 3. `resolve_scroll` - content-based detection + trust arbitration
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
        // Phase 1: Drain events, snapshot CDP hints
        let hints = self.snapshot_cdp_hints(
            now,
            editable_hint_hold_ms,
            key_input_qoi_boost_ms,
            editable_qoi_tile_margin,
        );

        // Phase 2: Track CDP scroll displacement
        let cdp_scroll_dy_px = self.track_cdp_scroll(
            &hints.cdp_hint_snapshot,
            max_cdp_scroll_dy_px,
        );

        // Phase 3: Resolve scroll from CDP + content sources
        let mut result = self.resolve_scroll(
            rgba,
            stride,
            now,
            &hints.cdp_hint_snapshot,
            hints.cdp_video_region_hint_sized,
            hints.pending_scrolls,
            hints.pending_scroll_dy_sum,
            cdp_scroll_dy_px,
            cdp_content_dy_divergence_log_px,
            min_scroll_dy_px,
            input_min_scroll_dy_px,
            input_scroll_min_confidence,
            no_input_scroll_min_confidence,
            scroll_suppress_video_frames,
        );

        // Merge hint-phase outputs into the final result
        result.editable_qoi_tile_bounds = hints.editable_qoi_tile_bounds;
        result.key_input_qoi_boost = hints.key_input_qoi_boost;

        result
    }
}
