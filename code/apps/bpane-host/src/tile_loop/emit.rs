//! Tile emission: static/content split, tile encoding, and frame send.

use tokio::sync::mpsc;

use crate::capture::ffmpeg::CaptureRegion;
use crate::region::extend_dirty_with_tile_bounds;
use crate::scroll::{has_scroll_region_split, is_content_tile_in_scroll_region};
use crate::tiles::{self, Rect};

use super::frame_types::DetectedScrollFrame;

fn send_tile_batch(
    tile_tx: &mpsc::Sender<bpane_protocol::frame::Frame>,
    mut content_frames: Vec<bpane_protocol::frame::Frame>,
    static_frames: Vec<bpane_protocol::frame::Frame>,
) -> bool {
    let batch_end_frame = content_frames.pop();
    for frame in content_frames {
        if tile_tx.blocking_send(frame).is_err() {
            return false;
        }
    }
    for frame in static_frames {
        if tile_tx.blocking_send(frame).is_err() {
            return false;
        }
    }
    if let Some(frame) = batch_end_frame {
        if tile_tx.blocking_send(frame).is_err() {
            return false;
        }
    }
    true
}

fn capture_region_to_rect(region: CaptureRegion) -> Option<Rect> {
    Some(Rect::new(
        u16::try_from(region.x).ok()?,
        u16::try_from(region.y).ok()?,
        u16::try_from(region.w).ok()?,
        u16::try_from(region.h).ok()?,
    ))
}

fn effective_video_capture_region(
    committed_video_region: Option<CaptureRegion>,
    cdp_video_region_hint: Option<CaptureRegion>,
) -> Option<CaptureRegion> {
    committed_video_region.or(cdp_video_region_hint)
}

fn rect_tile_bounds(
    region: Rect,
    tile_size: u16,
    cols: u16,
    rows: u16,
) -> Option<(u16, u16, u16, u16)> {
    if tile_size == 0 || cols == 0 || rows == 0 || region.w == 0 || region.h == 0 {
        return None;
    }
    let ts = u32::from(tile_size);
    let max_col = u32::from(cols.saturating_sub(1));
    let max_row = u32::from(rows.saturating_sub(1));
    let x1 = u32::from(region.x) + u32::from(region.w.saturating_sub(1));
    let y1 = u32::from(region.y) + u32::from(region.h.saturating_sub(1));
    Some((
        (u32::from(region.x) / ts).min(max_col) as u16,
        (u32::from(region.y) / ts).min(max_row) as u16,
        (x1 / ts).min(max_col) as u16,
        (y1 / ts).min(max_row) as u16,
    ))
}

fn video_region_transition_bounds(
    previous_video_region: Option<Rect>,
    active_video_region: Option<Rect>,
    tile_size: u16,
    cols: u16,
    rows: u16,
) -> Option<(u16, u16, u16, u16)> {
    if previous_video_region == active_video_region {
        return None;
    }
    let repair_region = match (previous_video_region, active_video_region) {
        (Some(previous), Some(current)) => previous.union(&current),
        (Some(previous), None) => previous,
        (None, Some(current)) => current,
        (None, None) => return None,
    };
    rect_tile_bounds(repair_region, tile_size, cols, rows)
}

fn tile_rect_with_offset(coord: tiles::TileCoord, grid: &tiles::TileGrid, offset_y: u16) -> Rect {
    let ts = grid.tile_size;
    let x = coord.col.saturating_mul(ts);
    let raw_y = coord.row as i32 * ts as i32 - offset_y as i32;
    let y = raw_y.max(0) as u16;
    let end_y = ((raw_y + ts as i32).min(grid.screen_h as i32)).max(0) as u16;
    let h = end_y.saturating_sub(y);
    let w = ts.min(grid.screen_w.saturating_sub(x));
    if w == 0 || h == 0 {
        return Rect::new(0, 0, 0, 0);
    }
    Rect::new(x, y, w, h)
}

fn filter_video_owned_coords(
    coords: Vec<tiles::TileCoord>,
    grid: &tiles::TileGrid,
    offset_y: u16,
    active_video_region: Option<Rect>,
) -> Vec<tiles::TileCoord> {
    let Some(region) = active_video_region else {
        return coords;
    };
    coords
        .into_iter()
        .filter(|coord| !region.overlaps(&tile_rect_with_offset(*coord, grid, offset_y)))
        .collect()
}

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
        let active_video_region =
            effective_video_capture_region(self.region_committer.active, cdp_video_region_hint)
                .and_then(capture_region_to_rect);
        let previous_video_region = self.emitter.current_video_region();
        if let Some(bounds) = video_region_transition_bounds(
            previous_video_region,
            active_video_region,
            self.tile_size,
            self.grid.cols,
            self.grid.rows,
        ) {
            extend_dirty_with_tile_bounds(&mut all_dirty, bounds);
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
        let content_dirty = filter_video_owned_coords(
            content_dirty,
            &self.grid,
            self.grid_offset_y,
            active_video_region,
        );
        let static_dirty =
            filter_video_owned_coords(static_dirty, &self.grid, 0, active_video_region);

        let result = self.emitter.emit_frame(
            &rgba,
            stride,
            &content_dirty,
            &self.grid,
            self.grid_offset_y,
            editable_qoi_tile_bounds,
            active_video_region,
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
                active_video_region,
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
        if s.fills > 0 || s.qoi_tiles > 0 || s.cache_hits > 0 || scroll_residual_ratio.is_some() {
            let scroll_saved_rate_total = if self.scroll_potential_tiles_total > 0 {
                Some(
                    self.scroll_saved_tiles_total as f32 / self.scroll_potential_tiles_total as f32,
                )
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
                cdp_video_hint = active_video_region.is_some(),
                active_video_region = self.region_committer.active.is_some(),
                editable_qoi = editable_qoi_tile_bounds.is_some(),
                key_input_qoi_boost,
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

        // Send all tile data (content + static) before BatchEnd so the client
        // applies the entire frame in a single batch.
        send_tile_batch(&self.tile_tx, result.tile_frames, static_frames)
    }
}

#[cfg(test)]
mod tests {
    use bpane_protocol::channel::ChannelId;
    use bpane_protocol::frame::Frame;
    use tokio::sync::mpsc;

    use crate::capture::ffmpeg::CaptureRegion;
    use crate::tiles::Rect;

    use crate::tiles::{TileCoord, TileGrid};

    use super::{
        effective_video_capture_region, filter_video_owned_coords, send_tile_batch,
        tile_rect_with_offset, video_region_transition_bounds,
    };

    #[test]
    fn send_tile_batch_keeps_batch_end_last() {
        let (tx, mut rx) = mpsc::channel(8);
        let content_frames = vec![
            Frame::new(ChannelId::Tiles, vec![1]),
            Frame::new(ChannelId::Tiles, vec![2]),
            Frame::new(ChannelId::Tiles, vec![3]), // BatchEnd
        ];
        let static_frames = vec![
            Frame::new(ChannelId::Tiles, vec![4]),
            Frame::new(ChannelId::Tiles, vec![5]),
        ];

        assert!(send_tile_batch(&tx, content_frames, static_frames));

        let mut payloads = Vec::new();
        while let Ok(frame) = rx.try_recv() {
            payloads.push(frame.payload);
        }
        assert_eq!(payloads, vec![vec![1], vec![2], vec![4], vec![5], vec![3]]);
    }

    #[test]
    fn video_region_transition_bounds_union_changed_regions() {
        let bounds = video_region_transition_bounds(
            Some(Rect::new(0, 0, 64, 64)),
            Some(Rect::new(64, 0, 64, 64)),
            64,
            4,
            4,
        );
        assert_eq!(bounds, Some((0, 0, 1, 0)));
    }

    #[test]
    fn video_region_transition_bounds_return_old_region_on_clear() {
        let bounds =
            video_region_transition_bounds(Some(Rect::new(64, 64, 64, 64)), None, 64, 4, 4);
        assert_eq!(bounds, Some((1, 1, 1, 1)));
    }

    #[test]
    fn effective_video_capture_region_prefers_committed_region() {
        let committed = CaptureRegion {
            x: 64,
            y: 64,
            w: 256,
            h: 128,
        };
        let hint = CaptureRegion {
            x: 0,
            y: 0,
            w: 128,
            h: 64,
        };
        assert_eq!(
            effective_video_capture_region(Some(committed), Some(hint)),
            Some(committed)
        );
    }

    #[test]
    fn effective_video_capture_region_falls_back_to_live_hint() {
        let hint = CaptureRegion {
            x: 0,
            y: 0,
            w: 128,
            h: 64,
        };
        assert_eq!(effective_video_capture_region(None, Some(hint)), Some(hint));
        assert_eq!(effective_video_capture_region(None, None), None);
    }

    #[test]
    fn filter_video_owned_coords_drops_overlapping_tiles() {
        let grid = TileGrid::new(256, 256, 64);
        let coords = vec![
            TileCoord::new(0, 0),
            TileCoord::new(2, 2),
            TileCoord::new(3, 3),
        ];
        let filtered = filter_video_owned_coords(coords, &grid, 0, Some(Rect::new(0, 0, 192, 192)));
        assert_eq!(filtered, vec![TileCoord::new(3, 3)]);
    }

    #[test]
    fn filter_video_owned_coords_respects_grid_offset() {
        let grid = TileGrid::new(128, 128, 64);
        let rect = tile_rect_with_offset(TileCoord::new(0, 1), &grid, 32);
        assert_eq!(rect, Rect::new(0, 32, 64, 64));

        let coords = vec![TileCoord::new(0, 1), TileCoord::new(1, 1)];
        let filtered = filter_video_owned_coords(coords, &grid, 32, Some(Rect::new(0, 32, 64, 64)));
        assert_eq!(filtered, vec![TileCoord::new(1, 1)]);
    }
}
