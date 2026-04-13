//! Scroll residual analysis: apply partition results, manage fallback,
//! thin mode, and repair scheduling.

use tracing::trace;

use crate::scroll::{build_scroll_exposed_strip_emit_coords, offset_tile_rect_for_emit};
use crate::tiles;

use super::frame_types::DetectedScrollFrame;
use super::scroll_partition::partition_and_compare;

/// Intermediate result from scroll residual analysis.
pub(crate) struct ScrollResidualResult {
    pub all_dirty: Vec<tiles::TileCoord>,
    pub scroll_residual_ratio: Option<f32>,
    pub scroll_residual_fallback_full: bool,
    pub scroll_residual_tiles_frame: Option<usize>,
    pub scroll_potential_tiles_frame: Option<usize>,
    pub scroll_saved_tiles_frame: Option<usize>,
    pub scroll_saved_ratio_frame: Option<f32>,
    pub scroll_emit_ratio_frame: Option<f32>,
    pub scroll_thin_mode_frame: bool,
    pub scroll_thin_repair_frame: bool,
}

impl super::TileCaptureThread {
    /// Analyze scroll residual: partition content/chrome, build residual,
    /// compute ratios, manage thin mode and repair scheduling.
    #[allow(clippy::too_many_lines)]
    pub(crate) fn analyze_scroll_residual(
        &mut self,
        rgba: &[u8],
        stride: usize,
        full_emit_coords: &[tiles::TileCoord],
        mut all_dirty: Vec<tiles::TileCoord>,
        detected_scroll_frame: &Option<DetectedScrollFrame>,
        scroll_residual_full_repaint_ratio: f32,
        scroll_thin_mode_residual_ratio: f32,
        scroll_thin_repair_quiet_frames: u8,
    ) -> ScrollResidualResult {
        let mut scroll_residual_ratio: Option<f32> = None;
        let mut scroll_residual_fallback_full = false;
        let mut scroll_residual_tiles_frame: Option<usize> = None;
        let mut scroll_potential_tiles_frame: Option<usize> = None;
        let mut scroll_saved_tiles_frame: Option<usize> = None;
        let mut scroll_saved_ratio_frame: Option<f32> = None;
        let mut scroll_emit_ratio_frame: Option<f32> = None;
        let mut scroll_thin_mode_frame = false;
        let mut scroll_thin_repair_frame = false;
        let detected_scroll_dy_px = detected_scroll_frame.as_ref().map(|d| d.dy);
        let prev_for_analysis = self.prev_frame.as_deref();
        if let (Some(_scroll_dy), Some(prev)) = (detected_scroll_dy_px, prev_for_analysis) {
            let dsf = detected_scroll_frame.as_ref().unwrap();
            let p = partition_and_compare(
                rgba, prev, stride, &self.grid, self.grid_offset_y,
                self.tile_size, self.scroll_copy_quantum_px,
                dsf.dy, full_emit_coords, dsf,
                self.last_scroll_region_top, self.screen_h,
                self.last_scroll_region_right, self.screen_w,
            );
            self.scroll_residual_batches_total = self.scroll_residual_batches_total.saturating_add(1);
            self.scroll_potential_tiles_total =
                self.scroll_potential_tiles_total.saturating_add(p.potential_tiles as u64);
            self.scroll_residual_tiles_total =
                self.scroll_residual_tiles_total.saturating_add(p.residual_tiles as u64);
            scroll_residual_tiles_frame = Some(p.residual_tiles);
            scroll_potential_tiles_frame = Some(p.potential_tiles);
            scroll_residual_ratio = Some(p.residual_ratio);

            if !p.quantized_scroll_copy {
                trace!(
                    dy = p.scroll_dy, scroll_copy_quantum_px = self.scroll_copy_quantum_px,
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
                all_dirty = full_emit_coords.to_vec();
            } else if p.interior_ratio > scroll_residual_full_repaint_ratio
                && !p.defer_scroll_repair
            {
                trace!(
                    dy = p.scroll_dy, row_shift = p.scroll_row_shift,
                    interior_ratio = format!("{:.2}", p.interior_ratio),
                    saved_tiles = p.saved_tiles, potential_tiles = p.potential_tiles,
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
                all_dirty = full_emit_coords.to_vec();
            } else {
                all_dirty = p.residual;
                all_dirty.extend(p.chrome_emit_coords.iter().copied());
                // Force content tiles overlapping the exposed strip into dirty set.
                {
                    let (exp_start, exp_end) = if p.scroll_dy > 0 {
                        let start = (p.srb_for_split as i32 - p.scroll_dy as i32)
                            .max(p.srt_for_split as i32);
                        (start, p.srb_for_split as i32)
                    } else {
                        let end = (p.srt_for_split as i32 + (-p.scroll_dy) as i32)
                            .min(p.srb_for_split as i32);
                        (p.srt_for_split as i32, end)
                    };
                    for &coord in &p.content_emit_coords {
                        if all_dirty.contains(&coord) { continue; }
                        let rect = offset_tile_rect_for_emit(coord, &self.grid, self.grid_offset_y);
                        if rect.w == 0 || rect.h == 0 { continue; }
                        let tile_top = rect.y as i32;
                        let tile_bot = tile_top + rect.h as i32;
                        if tile_bot > exp_start && tile_top < exp_end {
                            all_dirty.push(coord);
                        }
                    }
                }
                self.scroll_saved_tiles_total =
                    self.scroll_saved_tiles_total.saturating_add(p.saved_tiles as u64);
                scroll_saved_tiles_frame = Some(p.saved_tiles);
                if p.potential_tiles > 0 {
                    scroll_saved_ratio_frame =
                        Some(p.saved_tiles as f32 / p.potential_tiles as f32);
                    scroll_emit_ratio_frame =
                        Some(p.residual_tiles as f32 / p.potential_tiles as f32);
                } else {
                    scroll_saved_ratio_frame = Some(0.0);
                    scroll_emit_ratio_frame = Some(1.0);
                }
                self.scroll_residual_was_active = p.saved_tiles > 0;
                if p.defer_scroll_repair {
                    trace!(
                        dy = p.scroll_dy, row_shift = p.scroll_row_shift,
                        interior_ratio = format!("{:.2}", p.interior_ratio),
                        saved_tiles = p.saved_tiles, potential_tiles = p.potential_tiles,
                        "scroll copy accepted with deferred repair"
                    );
                }
                // Thin mode management
                let sub_tile_scroll = (p.scroll_dy.unsigned_abs() as u16) < self.tile_size;
                let residual_tiles_min_for_thin = (self.grid.cols as usize * 2).max(12);
                let residual_large_for_sub_tile = p.residual_ratio
                    >= scroll_thin_mode_residual_ratio
                    || p.residual_tiles >= residual_tiles_min_for_thin;
                if sub_tile_scroll && residual_large_for_sub_tile {
                    let residual_coords = &p.content_emit_coords;
                    let strip_dirty = build_scroll_exposed_strip_emit_coords(
                        &self.grid, self.grid_offset_y, p.scroll_dy, residual_coords,
                    );
                    if self.scroll_thin_mode_enabled && !strip_dirty.is_empty() {
                        all_dirty = strip_dirty;
                        all_dirty.extend(p.chrome_emit_coords.iter().copied());
                        self.scroll_thin_mode_active = true;
                        scroll_thin_mode_frame = true;
                        self.scroll_thin_batches_total =
                            self.scroll_thin_batches_total.saturating_add(1);
                    } else {
                        self.scroll_thin_mode_active = false;
                    }
                } else {
                    self.scroll_thin_mode_active = false;
                }
            }
        } else if (self.scroll_thin_mode_active || self.scroll_residual_was_active)
            && self.scroll_quiet_frames >= scroll_thin_repair_quiet_frames
        {
            all_dirty = full_emit_coords.to_vec();
            self.scroll_thin_mode_active = false;
            self.scroll_residual_was_active = false;
            scroll_thin_repair_frame = true;
            self.scroll_thin_repairs_total = self.scroll_thin_repairs_total.saturating_add(1);
        }

        ScrollResidualResult {
            all_dirty, scroll_residual_ratio, scroll_residual_fallback_full,
            scroll_residual_tiles_frame, scroll_potential_tiles_frame,
            scroll_saved_tiles_frame, scroll_saved_ratio_frame,
            scroll_emit_ratio_frame, scroll_thin_mode_frame, scroll_thin_repair_frame,
        }
    }
}
