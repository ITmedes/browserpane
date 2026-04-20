//! Scroll frame emission: send ScrollCopy, GridOffset, ScrollStats
//! frames via tile_tx, plus diagnostic logging.

use super::frame_types::{DetectedScrollFrame, ScrollEmitResult};
use crate::scroll::should_emit_scroll_copy;

use super::scroll_residual::ScrollResidualResult;

impl super::TileCaptureThread {
    /// Emit ScrollCopy, GridOffset, and ScrollStats frames via tile_tx.
    /// Returns the final `ScrollEmitResult`.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn send_scroll_frames(
        &mut self,
        detected_scroll_frame: Option<DetectedScrollFrame>,
        pending_scrolls: i32,
        pending_scroll_dy_sum: i32,
        input_scroll_dir: i32,
        cdp_scroll_dy_px: Option<i16>,
        residual: ScrollResidualResult,
    ) -> ScrollEmitResult {
        let emit_scroll_copy = should_emit_scroll_copy(
            residual.scroll_residual_fallback_full,
            residual.scroll_saved_tiles_frame,
        );
        if detected_scroll_frame.is_some()
            && !residual.scroll_residual_fallback_full
            && matches!(residual.scroll_saved_tiles_frame, Some(0))
        {
            self.scroll_zero_saved_batches_total =
                self.scroll_zero_saved_batches_total.saturating_add(1);
        }
        if let Some(ref dsf) = detected_scroll_frame {
            if emit_scroll_copy {
                if dsf.row_shift != 0 {
                    self.emitter.shift_hashes(dsf.row_shift, self.grid.rows);
                }
                self.emitter.zero_exposed_strip(
                    dsf.dy,
                    dsf.region_top,
                    dsf.region_bottom,
                    self.tile_size,
                    self.grid_offset_y,
                );
                let scroll_frame = bpane_protocol::TileMessage::ScrollCopy {
                    dx: 0,
                    dy: dsf.dy,
                    region_top: dsf.region_top,
                    region_bottom: dsf.region_bottom,
                    region_right: dsf.region_right,
                }
                .to_frame();
                if self.tile_tx.blocking_send(scroll_frame).is_err() {
                    return ScrollEmitResult {
                        all_dirty: residual.all_dirty,
                        detected_scroll_frame,
                        scroll_residual_ratio: residual.scroll_residual_ratio,
                        scroll_residual_fallback_full: residual.scroll_residual_fallback_full,
                        scroll_residual_tiles_frame: residual.scroll_residual_tiles_frame,
                        scroll_potential_tiles_frame: residual.scroll_potential_tiles_frame,
                        scroll_saved_tiles_frame: residual.scroll_saved_tiles_frame,
                        scroll_saved_ratio_frame: residual.scroll_saved_ratio_frame,
                        scroll_emit_ratio_frame: residual.scroll_emit_ratio_frame,
                        scroll_thin_mode_frame: residual.scroll_thin_mode_frame,
                        scroll_thin_repair_frame: residual.scroll_thin_repair_frame,
                    };
                }
            }

            let offset_frame = bpane_protocol::TileMessage::GridOffset {
                offset_x: 0,
                offset_y: self.grid_offset_y as i16,
            }
            .to_frame();
            if self.tile_tx.blocking_send(offset_frame).is_err() {
                return ScrollEmitResult {
                    all_dirty: residual.all_dirty,
                    detected_scroll_frame,
                    scroll_residual_ratio: residual.scroll_residual_ratio,
                    scroll_residual_fallback_full: residual.scroll_residual_fallback_full,
                    scroll_residual_tiles_frame: residual.scroll_residual_tiles_frame,
                    scroll_potential_tiles_frame: residual.scroll_potential_tiles_frame,
                    scroll_saved_tiles_frame: residual.scroll_saved_tiles_frame,
                    scroll_saved_ratio_frame: residual.scroll_saved_ratio_frame,
                    scroll_emit_ratio_frame: residual.scroll_emit_ratio_frame,
                    scroll_thin_mode_frame: residual.scroll_thin_mode_frame,
                    scroll_thin_repair_frame: residual.scroll_thin_repair_frame,
                };
            }

            let cdp_scroll_dir = cdp_scroll_dy_px.map(|dy| (dy as i32).signum()).unwrap_or(0);
            let min_scroll_dy_px = if pending_scrolls > 0 { 4 } else { 2 };
            tracing::debug!(
                source = dsf.source,
                dy = dsf.dy,
                confidence = format!("{:.2}", dsf.confidence),
                offset_y = self.grid_offset_y,
                row_shift = dsf.row_shift,
                scroll_copy = emit_scroll_copy,
                scrolls = pending_scrolls,
                input_dy_sum = pending_scroll_dy_sum,
                input_dir = input_scroll_dir,
                cdp_dir = cdp_scroll_dir,
                dir_match = dsf.direction_matches,
                min_scroll_dy_px,
                min_confidence = dsf.min_confidence.map(|c| format!("{:.2}", c)),
                "scroll detected"
            );
        }

        // Export cumulative host-side residual telemetry to client.
        let to_u32_sat = |v: u64| -> u32 { v.min(u32::MAX as u64) as u32 };
        let scroll_stats_frame = bpane_protocol::TileMessage::ScrollStats {
            scroll_batches_total: to_u32_sat(self.scroll_residual_batches_total),
            scroll_full_fallbacks_total: to_u32_sat(self.scroll_residual_fallback_full_total),
            scroll_potential_tiles_total: to_u32_sat(self.scroll_potential_tiles_total),
            scroll_saved_tiles_total: to_u32_sat(self.scroll_saved_tiles_total),
            scroll_non_quantized_fallbacks_total: to_u32_sat(
                self.scroll_fallback_non_quantized_total,
            ),
            scroll_residual_full_repaints_total: to_u32_sat(
                self.scroll_fallback_residual_full_repaint_total,
            ),
            scroll_zero_saved_batches_total: to_u32_sat(self.scroll_zero_saved_batches_total),
            host_sent_hash_entries: to_u32_sat(self.emitter.sent_hash_entries() as u64),
            host_sent_hash_evictions_total: to_u32_sat(self.emitter.sent_hash_evictions_total()),
            host_cache_miss_reports_total: to_u32_sat(self.client_cache_miss_reports_total),
        }
        .to_frame();
        if self.tile_tx.blocking_send(scroll_stats_frame).is_err() {
            return ScrollEmitResult {
                all_dirty: residual.all_dirty,
                detected_scroll_frame,
                scroll_residual_ratio: residual.scroll_residual_ratio,
                scroll_residual_fallback_full: residual.scroll_residual_fallback_full,
                scroll_residual_tiles_frame: residual.scroll_residual_tiles_frame,
                scroll_potential_tiles_frame: residual.scroll_potential_tiles_frame,
                scroll_saved_tiles_frame: residual.scroll_saved_tiles_frame,
                scroll_saved_ratio_frame: residual.scroll_saved_ratio_frame,
                scroll_emit_ratio_frame: residual.scroll_emit_ratio_frame,
                scroll_thin_mode_frame: residual.scroll_thin_mode_frame,
                scroll_thin_repair_frame: residual.scroll_thin_repair_frame,
            };
        }

        ScrollEmitResult {
            all_dirty: residual.all_dirty,
            detected_scroll_frame,
            scroll_residual_ratio: residual.scroll_residual_ratio,
            scroll_residual_fallback_full: residual.scroll_residual_fallback_full,
            scroll_residual_tiles_frame: residual.scroll_residual_tiles_frame,
            scroll_potential_tiles_frame: residual.scroll_potential_tiles_frame,
            scroll_saved_tiles_frame: residual.scroll_saved_tiles_frame,
            scroll_saved_ratio_frame: residual.scroll_saved_ratio_frame,
            scroll_emit_ratio_frame: residual.scroll_emit_ratio_frame,
            scroll_thin_mode_frame: residual.scroll_thin_mode_frame,
            scroll_thin_repair_frame: residual.scroll_thin_repair_frame,
        }
    }
}
