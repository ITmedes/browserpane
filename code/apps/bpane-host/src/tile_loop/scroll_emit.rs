//! Scroll residual analysis, thin-mode management, and dirty-set
//! computation for the current frame.

use tracing::{debug, trace};

use crate::scroll::{
    build_scroll_exposed_strip_emit_coords, build_scroll_residual_emit_coords,
    can_emit_scroll_copy, has_scroll_region_split, is_content_tile_in_scroll_region,
    next_scroll_active_capture_frames, offset_tile_rect_for_emit, should_defer_scroll_repair,
    should_emit_scroll_copy,
};
use crate::tiles;

use super::frame_types::{DetectedScrollFrame, ScrollEmitResult};

impl super::TileCaptureThread {
    /// Compute scroll residual, manage thin mode, and build the dirty tile set.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn compute_scroll_and_dirty(
        &mut self,
        rgba: &[u8],
        stride: usize,
        now: std::time::Instant,
        force_refresh: bool,
        detected_scroll_frame: Option<DetectedScrollFrame>,
        pending_scrolls: i32,
        pending_scroll_dy_sum: i32,
        input_scroll_dir: i32,
        cdp_scroll_dy_px: Option<i16>,
        strong_scroll_observed: bool,
        scroll_residual_full_repaint_ratio: f32,
        scroll_thin_mode_residual_ratio: f32,
        scroll_thin_repair_quiet_frames: u8,
    ) -> ScrollEmitResult {
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
        let detected_scroll_dy_px = detected_scroll_frame.as_ref().map(|d| d.dy);
        let prev_for_analysis = self.prev_frame.as_deref();
        if let (Some(scroll_dy), Some(prev)) = (detected_scroll_dy_px, prev_for_analysis) {
            let scroll_row_shift = detected_scroll_frame
                .as_ref()
                .map(|dsf| dsf.row_shift)
                .unwrap_or(0);
            // Get scroll region bounds for partitioning tiles into
            // content (scrollable) and chrome (static).  Use freshly
            // detected values when available, fall back to cached.
            let srt_for_split = detected_scroll_frame
                .as_ref()
                .map(|dsf| dsf.region_top)
                .unwrap_or(self.last_scroll_region_top);
            let srb_for_split = detected_scroll_frame
                .as_ref()
                .map(|dsf| dsf.region_bottom)
                .unwrap_or(self.screen_h);
            let srr_for_split = detected_scroll_frame
                .as_ref()
                .map(|dsf| dsf.region_right)
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
            } else if interior_ratio > scroll_residual_full_repaint_ratio
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
                    >= scroll_thin_mode_residual_ratio
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
            && self.scroll_quiet_frames >= scroll_thin_repair_quiet_frames
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
        if let Some(ref dsf) = detected_scroll_frame {
            if emit_scroll_copy {
                // Only shift hashes when ScrollCopy is actually sent —
                // keeps last_hashes consistent with the client canvas.
                if dsf.row_shift != 0 {
                    self.emitter.shift_hashes(dsf.row_shift, self.grid.rows);
                }
                // Always zero exposed strip when sending ScrollCopy,
                // even for sub-tile scrolls (row_shift == 0).  The client
                // keeps that strip stale until repair tiles arrive; if we
                // don't zero the corresponding hashes, L1 skip can keep
                // the stale strip visible indefinitely.
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
                        all_dirty,
                        detected_scroll_frame,
                        scroll_residual_ratio,
                        scroll_residual_fallback_full,
                        scroll_residual_tiles_frame,
                        scroll_potential_tiles_frame,
                        scroll_saved_tiles_frame,
                        scroll_saved_ratio_frame,
                        scroll_emit_ratio_frame,
                        scroll_thin_mode_frame,
                        scroll_thin_repair_frame,
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
                    all_dirty,
                    detected_scroll_frame,
                    scroll_residual_ratio,
                    scroll_residual_fallback_full,
                    scroll_residual_tiles_frame,
                    scroll_potential_tiles_frame,
                    scroll_saved_tiles_frame,
                    scroll_saved_ratio_frame,
                    scroll_emit_ratio_frame,
                    scroll_thin_mode_frame,
                    scroll_thin_repair_frame,
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
            return ScrollEmitResult {
                all_dirty,
                detected_scroll_frame,
                scroll_residual_ratio,
                scroll_residual_fallback_full,
                scroll_residual_tiles_frame,
                scroll_potential_tiles_frame,
                scroll_saved_tiles_frame,
                scroll_saved_ratio_frame,
                scroll_emit_ratio_frame,
                scroll_thin_mode_frame,
                scroll_thin_repair_frame,
            };
        }


        ScrollEmitResult {
            all_dirty,
            detected_scroll_frame,
            scroll_residual_ratio,
            scroll_residual_fallback_full,
            scroll_residual_tiles_frame,
            scroll_potential_tiles_frame,
            scroll_saved_tiles_frame,
            scroll_saved_ratio_frame,
            scroll_emit_ratio_frame,
            scroll_thin_mode_frame,
            scroll_thin_repair_frame,
        }
    }
}
