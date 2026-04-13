//! H.264 region management and video tile info updates per frame.

use std::sync::atomic::Ordering;

use bpane_protocol::VideoTileInfo;

use crate::capture::ffmpeg::CaptureRegion;
use crate::config::H264Mode;

/// Decision output from H.264 region update.
pub(crate) struct H264Update {
    pub tiles_cover_screen: bool,
}

impl super::TileCaptureThread {
    /// Update the FFmpeg capture region and H.264 enable/disable state.
    /// Returns whether tiles cover the full screen (i.e., H.264 is off).
    pub(crate) fn update_h264_region(
        &mut self,
        cdp_video_region_hint: Option<CaptureRegion>,
        cdp_has_video: bool,
        cdp_motion_tiles: u32,
        now: std::time::Instant,
        region_reconfig_stable_frames: u8,
        region_reconfig_min_interval_ms: u64,
        min_changed_video_tiles_for_h264: u32,
        h264_min_on_duration_ms: u64,
    ) -> H264Update {
        let desired_h264 = match self.h264_mode {
            H264Mode::Always => true,
            H264Mode::VideoTiles => cdp_has_video,
            H264Mode::Off => false,
        };

        let next_capture_region = if matches!(self.h264_mode, H264Mode::VideoTiles) {
            cdp_has_video
                .then_some(cdp_video_region_hint)
                .flatten()
        } else {
            None
        };
        let committed =
            self.region_committer
                .commit(next_capture_region, region_reconfig_stable_frames);

        if desired_h264 && committed != self.region_committer.active {
            if self.region_committer.should_reconfig(
                committed,
                now,
                region_reconfig_min_interval_ms,
            ) {
                self.region_committer.apply_reconfig(committed, now);
                let _ = self
                    .cmd_tx
                    .send(crate::capture::ffmpeg::PipelineCmd::SetRegion(committed));
            }
        } else if !desired_h264 {
            self.region_committer.active = committed;
        }

        // Update shared video tile info
        let next_tile_info = if matches!(self.h264_mode, H264Mode::VideoTiles) {
            self.region_committer.active.map(|region| VideoTileInfo {
                tile_x: region.x as u16,
                tile_y: region.y as u16,
                tile_w: region.w as u16,
                tile_h: region.h as u16,
                screen_w: self.screen_w,
                screen_h: self.screen_h,
            })
        } else {
            None
        };
        {
            let mut guard = match self.video_tile_info.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            *guard = next_tile_info;
        }

        // H.264 toggle with minimum on-duration
        let mut effective_h264 = desired_h264;
        if self.h264_enabled
            && !desired_h264
            && now.duration_since(self.last_h264_toggle_at)
                < std::time::Duration::from_millis(h264_min_on_duration_ms)
        {
            effective_h264 = true;
        }
        if effective_h264 != self.h264_enabled {
            self.h264_enabled = effective_h264;
            self.last_h264_toggle_at = now;
            let _ = self
                .cmd_tx
                .send(crate::capture::ffmpeg::PipelineCmd::SetEnabled(effective_h264));
        }

        let tiles_cover_screen = if matches!(self.h264_mode, H264Mode::Off) {
            true
        } else {
            !effective_h264 || self.region_committer.active.is_none()
        };
        self.tiles_active
            .store(tiles_cover_screen, Ordering::Relaxed);

        H264Update {
            tiles_cover_screen,
        }
    }
}
