//! CDP hint snapshot: drain input events, read browser hint state,
//! compute video/editable region hints and editable QOI tile bounds.

use crate::cdp_video;
use crate::region::{
    capture_region_tile_bounds, clamp_region_to_screen, expand_tile_bounds,
    region_meets_editable_minimum, region_meets_video_minimum,
};

use super::frame_types::CdpHintSnapshot;

impl super::TileCaptureThread {
    /// Drain pending input events, read the browser CDP hint mutex, and
    /// compute sized video/editable region hints plus editable QOI tile bounds.
    pub(crate) fn snapshot_cdp_hints(
        &mut self,
        now: std::time::Instant,
        editable_hint_hold_ms: u64,
        key_input_qoi_boost_ms: u64,
        editable_qoi_tile_margin: u16,
    ) -> CdpHintSnapshot {
        // Drain pending scroll events (used only for logging).
        let mut pending_scrolls = 0i32;
        let mut pending_scroll_dy_sum = 0i32;
        while let Ok((_, dy)) = self.scroll_rx.try_recv() {
            pending_scrolls += 1;
            pending_scroll_dy_sum += dy as i32;
        }
        while let Ok((x, y, ts)) = self.video_click_rx.try_recv() {
            self.last_left_click = Some((x, y, ts));
        }
        while let Ok(ts) = self.text_input_rx.try_recv() {
            self.editable_hint.on_key_input(ts);
        }

        let cdp_hint_snapshot = {
            let guard = match self.browser_video_hint.lock() {
                Ok(g) => g,
                Err(poisoned) => poisoned.into_inner(),
            };
            *guard
        };
        let cdp_hint_region_kind = cdp_hint_snapshot.region_kind;
        let cdp_hint_region_raw = cdp_hint_snapshot.video_region.and_then(|region| {
            clamp_region_to_screen(region, self.screen_w as u32, self.screen_h as u32)
        });
        let cdp_video_region_hint_sized =
            if matches!(cdp_hint_region_kind, cdp_video::HintRegionKind::Video) {
                cdp_hint_region_raw.filter(|region| {
                    region_meets_video_minimum(
                        region.w,
                        region.h,
                        self.screen_w as u32,
                        self.screen_h as u32,
                        self.min_cdp_video_width_px,
                        self.min_cdp_video_height_px,
                        self.min_cdp_video_area_ratio,
                    )
                })
            } else {
                None
            };
        let cdp_editable_region_hint =
            if matches!(cdp_hint_region_kind, cdp_video::HintRegionKind::Editable) {
                cdp_hint_region_raw
                    .filter(|region| region_meets_editable_minimum(region.w, region.h))
            } else {
                None
            };
        self.editable_hint.update(cdp_editable_region_hint, now, editable_hint_hold_ms);
        let key_input_qoi_boost = self.editable_hint.key_input_qoi_boost(now, key_input_qoi_boost_ms);
        let editable_qoi_region = self.editable_hint.qoi_region(
            cdp_editable_region_hint,
            now,
            editable_hint_hold_ms,
            key_input_qoi_boost_ms,
        );
        let editable_qoi_tile_bounds = editable_qoi_region.map(|region| {
            expand_tile_bounds(
                capture_region_tile_bounds(region, self.tile_size, self.grid.cols, self.grid.rows),
                editable_qoi_tile_margin,
                self.grid.cols,
                self.grid.rows,
            )
        });

        CdpHintSnapshot {
            cdp_hint_snapshot,
            cdp_video_region_hint_sized,
            cdp_editable_region_hint,
            editable_qoi_tile_bounds,
            key_input_qoi_boost,
            pending_scrolls,
            pending_scroll_dy_sum,
        }
    }
}
