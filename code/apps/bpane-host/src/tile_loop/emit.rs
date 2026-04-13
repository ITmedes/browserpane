//! Tile emission: static/content split, tile encoding, and frame send.

use tracing::trace;

use crate::capture::ffmpeg::CaptureRegion;
use crate::region::extend_dirty_with_tile_bounds;
use crate::scroll::{has_scroll_region_split, is_content_tile_in_scroll_region};
use crate::tiles;

use super::frame_types::DetectedScrollFrame;

impl super::TileCaptureThread {
    /// Split dirty tiles into content/static, emit tile frames, and send.
    ///
    /// Returns `false` if the tile channel is closed (caller should exit).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn emit_tiles(
        &mut self,
        rgba: &[u8],
        stride: usize,
        mut all_dirty: Vec<tiles::TileCoord>,
        detected_scroll_frame: &Option<DetectedScrollFrame>,
        latched_video_tiles: &[tiles::TileCoord],
        cdp_video_region_hint: Option<CaptureRegion>,
        cdp_motion_tiles: u32,
        editable_qoi_tile_bounds: Option<(u16, u16, u16, u16)>,
        key_input_qoi_boost: bool,
        scroll_residual_ratio: Option<f32>,
        scroll_residual_fallback_full: bool,
        scroll_residual_tiles_frame: Option<usize>,
        scroll_potential_tiles_frame: Option<usize>,
        scroll_saved_tiles_frame: Option<usize>,
        scroll_saved_ratio_frame: Option<f32>,
        scroll_emit_ratio_frame: Option<f32>,
        scroll_thin_mode_frame: bool,
        scroll_thin_repair_frame: bool,
        min_changed_video_tiles_for_h264: u32,
    ) -> bool {
        // Split dirty tiles into static (browser chrome) and content (scrolling
        // viewport). Static tiles are emitted at raw framebuffer positions
        // with a separate hash table that is never shifted, so browser
        // header/scrollbar tiles achieve L1 cache hits across scroll frames.
        if let Some(dsf) = detected_scroll_frame {
            self.last_scroll_region_top = dsf.region_top;
            self.last_scroll_region_bottom = dsf.region_bottom;
            self.last_scroll_region_right = dsf.region_right;
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
                cdp_hint_raw = cdp_video_region_hint.is_some(),
                cdp_video_hint = cdp_video_region_hint.is_some(),
                editable_qoi = editable_qoi_tile_bounds.is_some(),
                key_input_qoi_boost,
                self.click_armed = cdp_video_region_hint.is_some(),
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


        true // channel still open
    }
}
