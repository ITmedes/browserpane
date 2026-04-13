//! CDP scroll tracking: extract scroll displacement from CDP scrollY
//! and scroll_delta_y, update sequence tracking.

use crate::region::scale_css_px_to_screen_px;

impl super::TileCaptureThread {
    /// Extract CDP-reported scroll displacement from absolute scrollY
    /// and incremental scroll_delta_y, updating sequence tracking state.
    ///
    /// Returns `cdp_scroll_dy_px` if a non-zero scroll was detected.
    pub(crate) fn track_cdp_scroll(
        &mut self,
        cdp_hint_snapshot: &crate::cdp_video::PageHintState,
        max_cdp_scroll_dy_px: i64,
    ) -> Option<i16> {
        let mut cdp_scroll_dy_px: Option<i16> = None;
        let cdp_scale_milli = cdp_hint_snapshot.device_scale_factor_milli.max(1);
        if let Some(scroll_y_css) = cdp_hint_snapshot.scroll_y {
            let scroll_y = scale_css_px_to_screen_px(scroll_y_css, cdp_scale_milli);
            if let Some(prev_scroll_y) = self.last_cdp_scroll_y {
                let dy = scroll_y.saturating_sub(prev_scroll_y);
                if dy != 0 {
                    let clamped = dy.clamp(-max_cdp_scroll_dy_px, max_cdp_scroll_dy_px) as i16;
                    if clamped != 0 {
                        cdp_scroll_dy_px = Some(clamped);
                    }
                }
            }
            self.last_cdp_scroll_y = Some(scroll_y);
        } else {
            self.last_cdp_scroll_y = None;
        }
        if cdp_scroll_dy_px.is_none()
            && cdp_hint_snapshot.update_seq != 0
            && cdp_hint_snapshot.update_seq != self.last_cdp_hint_seq
            && cdp_hint_snapshot.scroll_delta_y != 0
        {
            let clamped = scale_css_px_to_screen_px(
                cdp_hint_snapshot.scroll_delta_y as i64,
                cdp_scale_milli,
            )
            .clamp(-max_cdp_scroll_dy_px, max_cdp_scroll_dy_px)
                as i16;
            if clamped != 0 {
                cdp_scroll_dy_px = Some(clamped);
            }
        }
        if cdp_hint_snapshot.update_seq != 0 {
            self.last_cdp_hint_seq = cdp_hint_snapshot.update_seq;
        }
        cdp_scroll_dy_px
    }
}
