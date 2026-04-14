//! Two-pass per-tile video classification with motion features,
//! bounding-box stability, and hysteresis scoring.

use crate::capture::ffmpeg::CaptureRegion;
use crate::region::hash_tile_region;
use crate::tiles;
use crate::video_classify::{
    bbox_center_shift, bbox_iou, compute_tile_motion_features, is_photo_like_tile,
};

use super::frame_types::ClassifyResult;

impl super::TileCaptureThread {
    /// Run two-pass video classification on the current frame.
    ///
    /// Pass 1: hash tiles, extract motion features, identify candidates.
    /// Pass 2: update scores with hysteresis, latch/unlatch video tiles.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn classify_tiles(
        &mut self,
        rgba: &[u8],
        stride: usize,
        cdp_video_region_hint: Option<CaptureRegion>,
        cdp_hint_tile_bounds: Option<(u16, u16, u16, u16)>,
        video_enter_score: i8,
        video_exit_score: i8,
        video_max_score: i8,
        video_decay_streak: u8,
        video_min_hold_frames: u8,
        region_min_candidates: u32,
        region_dense_candidates: u32,
        min_video_bbox_width_tiles: u16,
        min_video_bbox_height_tiles: u16,
        min_video_bbox_area_ratio: f32,
    ) -> ClassifyResult {
        let prev_for_analysis = self.prev_frame.as_deref();
        // ── Per-tile change detection (two-pass) ─────────────
        // Pass 1: hash each tile, extract motion features for changed
        // tiles, identify video candidates, compute candidate bbox.
        let cols = self.grid.cols as usize;
        self.candidate_mask.fill(false);
        self.changed_mask.fill(false);
        self.text_like_mask.fill(false);
        let mut bbox_min_col: u16 = u16::MAX;
        let mut bbox_max_col: u16 = 0;
        let mut bbox_min_row: u16 = u16::MAX;
        let mut bbox_max_row: u16 = 0;
        let mut candidate_count = 0u32;
        let mut strong_candidate_count = 0u32;

        for row in 0..self.grid.rows {
            for col in 0..self.grid.cols {
                let idx = row as usize * cols + col as usize;
                let tx = col as usize * self.tile_size as usize;
                let ty = row as usize * self.tile_size as usize;
                let tw = (self.tile_size as usize).min(self.screen_w as usize - tx);
                let th = (self.tile_size as usize).min(self.screen_h as usize - ty);
                let hash = hash_tile_region(&rgba, stride, tx, ty, tw, th);
                let changed = self.prev_hashes[idx] != 0 && hash != self.prev_hashes[idx];
                self.changed_mask[idx] = changed;

                if changed
                    && self.video_classification_enabled
                    && cdp_video_region_hint.is_some()
                    && self.scroll_cooldown_frames == 0
                {
                    let features = compute_tile_motion_features(
                        &rgba,
                        prev_for_analysis,
                        stride,
                        tx,
                        ty,
                        tw,
                        th,
                    );
                    // Text/UI is typically edge-heavy and lower entropy.
                    // Video/canvas is usually high motion + richer entropy.
                    self.text_like_mask[idx] =
                        features.edge_density > 0.22 && features.entropy_hint < 0.45;
                    let photo_like = is_photo_like_tile(&rgba, stride, tx, ty, tw, th, 16);
                    let video_like = features.change_ratio > 0.23
                        && features.motion_magnitude > 0.045
                        && features.entropy_hint > 0.16
                        && features.edge_density < 0.74
                        && photo_like;
                    let strong_video_like = features.change_ratio > 0.30
                        && features.motion_magnitude > 0.07
                        && features.entropy_hint > 0.20
                        && features.edge_density < 0.70
                        && photo_like;

                    if video_like {
                        self.candidate_mask[idx] = true;
                        candidate_count += 1;
                        if strong_video_like {
                            strong_candidate_count += 1;
                        }
                        bbox_min_col = bbox_min_col.min(col);
                        bbox_max_col = bbox_max_col.max(col);
                        bbox_min_row = bbox_min_row.min(row);
                        bbox_max_row = bbox_max_row.max(row);
                    }
                }
                self.prev_hashes[idx] = hash;
            }
        }

        // Bounding box stability check:
        // Video regions stay roughly stable in position/shape, while
        // scrolling tends to translate the bbox frame to frame.
        let current_bbox = if candidate_count > 0 {
            Some((bbox_min_col, bbox_min_row, bbox_max_col, bbox_max_row))
        } else {
            None
        };

        let bbox_stable = match (current_bbox, self.prev_video_bbox) {
            (Some(cur), Some(prev)) => {
                let iou = bbox_iou(cur, prev);
                let center_shift = bbox_center_shift(cur, prev);
                iou > 0.55 || center_shift <= 1.25
            }
            _ => false,
        };
        if bbox_stable {
            self.stable_bbox_frames = self.stable_bbox_frames.saturating_add(1);
        } else {
            self.stable_bbox_frames = 0;
        }
        self.prev_video_bbox = current_bbox;
        let bbox_tile_area = current_bbox
            .map(|(min_c, min_r, max_c, max_r)| {
                ((max_c - min_c + 1) as u32).saturating_mul((max_r - min_r + 1) as u32)
            })
            .unwrap_or(0);
        let bbox_density = if bbox_tile_area > 0 {
            candidate_count as f32 / bbox_tile_area as f32
        } else {
            0.0
        };
        let (bbox_w_tiles, bbox_h_tiles) = current_bbox
            .map(|(min_c, min_r, max_c, max_r)| (max_c - min_c + 1, max_r - min_r + 1))
            .unwrap_or((0, 0));
        let total_tile_area = (self.grid.cols as u32)
            .saturating_mul(self.grid.rows as u32)
            .max(1);
        let bbox_area_ratio = bbox_tile_area as f32 / total_tile_area as f32;
        let large_stable_region = bbox_w_tiles >= min_video_bbox_width_tiles
            && bbox_h_tiles >= min_video_bbox_height_tiles
            && bbox_area_ratio >= min_video_bbox_area_ratio;
        let region_stable = self.video_classification_enabled
            && cdp_video_region_hint.is_some()
            && self.scroll_cooldown_frames == 0
            && candidate_count >= region_min_candidates
            && large_stable_region
            && ((self.stable_bbox_frames >= 4 && bbox_density > 0.34)
                || candidate_count >= region_dense_candidates
                || strong_candidate_count >= 8);

        // Pass 2: update per-tile motion scores + hysteresis and classify.
        let mut latched_video_tiles: Vec<tiles::TileCoord> = Vec::new();
        let mut cdp_motion_tiles: u32 = 0;
        for row in 0..self.grid.rows {
            for col in 0..self.grid.cols {
                let idx = row as usize * cols + col as usize;
                let in_cdp_video_hint = cdp_hint_tile_bounds
                    .map(|(min_col, min_row, max_col, max_row)| {
                        col >= min_col && col <= max_col && row >= min_row && row <= max_row
                    })
                    .unwrap_or(false);
                let cdp_motion_candidate =
                    in_cdp_video_hint && self.changed_mask[idx] && !self.text_like_mask[idx];
                if cdp_motion_candidate {
                    cdp_motion_tiles = cdp_motion_tiles.saturating_add(1);
                }
                if region_stable && self.candidate_mask[idx] {
                    self.video_scores[idx] = (self.video_scores[idx] + 4).min(video_max_score);
                    self.non_candidate_streaks[idx] = 0;
                    if self.video_latched[idx] {
                        self.video_hold_frames[idx] = video_min_hold_frames;
                    }
                } else if cdp_motion_candidate {
                    self.video_scores[idx] = (self.video_scores[idx] + 2).min(video_max_score);
                    self.non_candidate_streaks[idx] = 0;
                    if self.video_latched[idx] {
                        self.video_hold_frames[idx] = video_min_hold_frames;
                    }
                } else if self.changed_mask[idx] {
                    self.video_scores[idx] = (self.video_scores[idx] - 1).max(-video_max_score);
                    self.non_candidate_streaks[idx] =
                        self.non_candidate_streaks[idx].saturating_add(1);
                } else {
                    self.video_scores[idx] = (self.video_scores[idx] - 1).max(-video_max_score);
                    self.non_candidate_streaks[idx] =
                        self.non_candidate_streaks[idx].saturating_add(1);
                }

                if !(region_stable && self.candidate_mask[idx] || cdp_motion_candidate)
                    && self.video_hold_frames[idx] > 0
                {
                    self.video_hold_frames[idx] -= 1;
                }

                if !self.video_classification_enabled
                    || cdp_video_region_hint.is_none()
                    || self.scroll_cooldown_frames > 0
                {
                    self.video_latched[idx] = false;
                    self.video_hold_frames[idx] = 0;
                    self.video_scores[idx] = self.video_scores[idx].min(0);
                } else if self.video_latched[idx] {
                    if self.video_hold_frames[idx] == 0
                        && (self.video_scores[idx] <= video_exit_score
                            || self.non_candidate_streaks[idx] >= video_decay_streak)
                    {
                        self.video_latched[idx] = false;
                        self.video_hold_frames[idx] = 0;
                    }
                } else if (region_stable && self.video_scores[idx] >= video_enter_score)
                    || (cdp_motion_candidate && self.video_scores[idx] >= (video_enter_score - 1))
                {
                    self.video_latched[idx] = true;
                    self.video_hold_frames[idx] = video_min_hold_frames;
                }

                if let Some(tile) = self.grid.get_mut(tiles::TileCoord::new(col, row)) {
                    tile.dirty = true;
                    tile.classification = if self.video_latched[idx] {
                        tiles::TileClass::VideoMotion
                    } else if self.changed_mask[idx] && self.text_like_mask[idx] {
                        tiles::TileClass::TextScroll
                    } else {
                        tiles::TileClass::Static
                    };
                }
                if self.video_latched[idx] {
                    latched_video_tiles.push(tiles::TileCoord::new(col, row));
                }
            }
        }

        ClassifyResult {
            latched_video_tiles,
            cdp_motion_tiles,
        }
    }
}
