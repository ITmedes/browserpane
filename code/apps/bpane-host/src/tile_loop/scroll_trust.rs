//! Trust arbitration between CDP and content-based scroll sources.
//!
//! Given a CDP-reported scroll delta and an optional content-based
//! scroll measurement, select the most reliable source.

use tracing::trace;

use crate::scroll::select_wheel_trusted_scroll;

/// Select trusted scroll from CDP and content-based sources.
///
/// Returns `(dy, confidence, source_label, direction_matches, min_confidence)`.
#[allow(clippy::too_many_arguments)]
pub(super) fn arbitrate_scroll_trust(
    cdp_scroll_dy_px: Option<i16>,
    content_scroll: Option<(i16, f32, bool, Option<f32>)>,
    pending_scrolls: i32,
    pending_scroll_dy_sum: i32,
    input_scroll_dir: i32,
    cdp_scroll_dir: i32,
    min_scroll_dy_px: i32,
    cdp_content_dy_divergence_log_px: i32,
) -> Option<(i16, f32, &'static str, bool, Option<f32>)> {
    let mut trusted_scroll: Option<(i16, f32, &'static str, bool, Option<f32>)> = None;

    if let Some(detected_dy) = cdp_scroll_dy_px {
        let cdp_abs = (detected_dy as i32).abs();
        if cdp_abs >= min_scroll_dy_px {
            if pending_scrolls > 0 {
                if let Some((
                    selected_dy,
                    selected_confidence,
                    source,
                    direction_matches,
                    min_confidence,
                )) = select_wheel_trusted_scroll(detected_dy, input_scroll_dir, content_scroll)
                {
                    if source == "content" {
                        let dy_gap = ((selected_dy as i32) - (detected_dy as i32)).abs();
                        if dy_gap >= cdp_content_dy_divergence_log_px {
                            trace!(
                                cdp_dy = detected_dy,
                                content_dy = selected_dy,
                                dy_gap,
                                confidence = format!("{:.2}", selected_confidence),
                                "wheel scroll dy diverged; preferring content (pixel-aligned)"
                            );
                        }
                    }
                    trusted_scroll = Some((
                        selected_dy,
                        selected_confidence,
                        source,
                        direction_matches,
                        min_confidence,
                    ));
                } else {
                    let detected_scroll_dir = (detected_dy as i32).signum();
                    let direction_matches = input_scroll_dir == 0
                        || detected_scroll_dir == 0
                        || input_scroll_dir == detected_scroll_dir;
                    trace!(
                        source = "cdp",
                        dy = detected_dy,
                        scrolls = pending_scrolls,
                        input_dy_sum = pending_scroll_dy_sum,
                        input_dir = input_scroll_dir,
                        dir_match = direction_matches,
                        min_scroll_dy_px,
                        "ignored cdp scroll hint with mismatched direction"
                    );
                }
            } else if let Some((
                content_dy,
                content_confidence,
                content_direction_matches,
                content_min_confidence,
            )) = content_scroll
            {
                let cdp_dir = (detected_dy as i32).signum();
                let content_dir = (content_dy as i32).signum();
                if cdp_dir == 0 || content_dir == 0 || cdp_dir == content_dir {
                    let dy_gap = ((content_dy as i32) - (detected_dy as i32)).abs();
                    if dy_gap >= cdp_content_dy_divergence_log_px {
                        trace!(
                            cdp_dy = detected_dy,
                            content_dy,
                            dy_gap,
                            confidence = format!("{:.2}", content_confidence),
                            "cdp/content scroll dy diverged; preferring content (pixel-aligned)"
                        );
                    }
                    trusted_scroll = Some((
                        content_dy,
                        content_confidence,
                        "content",
                        content_direction_matches,
                        content_min_confidence,
                    ));
                } else {
                    trace!(
                        cdp_dy = detected_dy,
                        content_dy,
                        confidence = format!("{:.2}", content_confidence),
                        "cdp/content direction mismatch; preferring content"
                    );
                    trusted_scroll = Some((
                        content_dy,
                        content_confidence,
                        "content",
                        content_direction_matches,
                        content_min_confidence,
                    ));
                }
            } else {
                trace!(
                    source = "cdp-passive-skipped",
                    dy = detected_dy,
                    scrolls = pending_scrolls,
                    cdp_dir = cdp_scroll_dir,
                    "cdp scroll unconfirmed by content detection; skipping scroll mode"
                );
            }
        } else if pending_scrolls > 0 {
            trace!(
                source = "cdp",
                dy = detected_dy,
                scrolls = pending_scrolls,
                input_dy_sum = pending_scroll_dy_sum,
                input_dir = input_scroll_dir,
                min_scroll_dy_px,
                "ignored tiny cdp scroll hint"
            );
        }
    }

    if trusted_scroll.is_none() {
        if let Some((dy, confidence, direction_matches, min_confidence)) = content_scroll {
            trusted_scroll = Some((dy, confidence, "content", direction_matches, min_confidence));
        }
    }

    trusted_scroll
}
