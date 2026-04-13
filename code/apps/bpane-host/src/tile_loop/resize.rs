//! Resize handling for the tile capture thread.

use tracing::debug;

use crate::capture::CaptureBackend;
use crate::tiles;
use crate::video_region;

impl super::TileCaptureThread {
    /// Check for screen resolution changes and rebuild the grid if needed.
    /// Returns `true` if a resize occurred (caller should skip this frame).
    pub(crate) fn handle_resize(&mut self, now: std::time::Instant) -> bool {
        let (cur_w, cur_h) = if now.duration_since(self.last_resize_check)
            >= std::time::Duration::from_millis(500)
        {
            self.last_resize_check = now;
            self.cap.refresh_screen_size()
        } else {
            self.cap.query_screen_size()
        };

        if cur_w as u16 == self.screen_w && cur_h as u16 == self.screen_h {
            return false;
        }

        self.screen_w = cur_w as u16;
        self.screen_h = cur_h as u16;
        self.grid = tiles::TileGrid::new(self.screen_w, self.screen_h, self.tile_size);
        self.emitter = tiles::emitter::TileEmitter::with_codec(
            self.grid.cols,
            self.grid.rows,
            self.tile_codec,
        );

        let new_total = self.grid.cols as usize * self.grid.rows as usize;
        self.prev_hashes = vec![0; new_total];
        self.video_scores = vec![0; new_total];
        self.non_candidate_streaks = vec![0; new_total];
        self.video_hold_frames = vec![0; new_total];
        self.video_latched = vec![false; new_total];
        self.candidate_mask = vec![false; new_total];
        self.changed_mask = vec![false; new_total];
        self.text_like_mask = vec![false; new_total];
        self.prev_video_bbox = None;
        self.stable_bbox_frames = 0;
        self.scroll_cooldown_frames = 0;
        self.scroll_thin_mode_active = false;
        self.scroll_residual_was_active = false;
        self.scroll_quiet_frames = 0;
        self.last_cdp_hint_seq = 0;
        self.last_cdp_scroll_y = None;
        self.cdp_scroll_anchor = None;
        self.prev_frame = None;
        self.content_origin_y = 0;
        self.grid_offset_y = 0;
        self.last_scroll_region_top = 0;
        self.last_scroll_region_bottom = self.screen_h;
        self.last_scroll_region_right = self.screen_w;

        let had_region = self.region_committer.active.is_some();
        self.region_committer.reset();
        self.last_h264_toggle_at = now;
        if had_region {
            let _ = self
                .cmd_tx
                .send(crate::capture::ffmpeg::PipelineCmd::SetRegion(None));
        }
        {
            let mut guard = match self.video_tile_info.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            *guard = None;
        }

        debug!("tile capture: resized to {}x{}", self.screen_w, self.screen_h);

        let grid_frame = self.emitter.emit_grid_config(&self.grid);
        if self.tile_tx.blocking_send(grid_frame).is_err() {
            return true; // channel closed, will exit on next send attempt
        }

        true
    }
}
