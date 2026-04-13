//! Pixel-based vertical scroll detection via column sampling.

use super::CONTENT_SCROLL_SEARCH_MAX_PX;

/// Detect vertical scroll displacement by comparing pixel columns between frames.
///
/// Samples 5 columns at [10%, 30%, 50%, 70%, 90%] of screen width.
/// For each candidate dy, counts matching pixels across all columns.
/// Returns `(signed_dy, confidence)` if ≥3 of 5 columns agree.
pub fn detect_column_scroll(
    current: &[u8],
    previous: &[u8],
    stride: usize,
    width: usize,
    height: usize,
    max_search: usize,
) -> Option<(i32, f32)> {
    if height < 16
        || width < 16
        || current.len() < stride * height
        || previous.len() < stride * height
    {
        return None;
    }

    let max_dy = max_search.min(height / 2);
    let sample_xs: Vec<usize> = [10, 30, 50, 70, 90]
        .iter()
        .map(|pct| (*pct * width / 100).min(width - 1))
        .collect();

    let mut best_dy: i32 = 0;
    let mut best_total_matches: usize = 0;
    let mut best_agreeing_cols: usize = 0;

    for dy in 1..=max_dy {
        for sign in [1i32, -1i32] {
            let signed_dy = dy as i32 * sign;
            let mut total_matches = 0usize;
            let mut agreeing_cols = 0usize;
            let overlap = height - dy;

            for &col_x in &sample_xs {
                let byte_offset = col_x * 4;
                let mut col_matches = 0usize;

                for y in 0..overlap {
                    let (curr_y, prev_y) = if sign > 0 {
                        (y, y + dy)
                    } else {
                        (y + dy, y)
                    };

                    let curr_off = curr_y * stride + byte_offset;
                    let prev_off = prev_y * stride + byte_offset;

                    if curr_off + 4 <= current.len()
                        && prev_off + 4 <= previous.len()
                        && current[curr_off..curr_off + 4] == previous[prev_off..prev_off + 4]
                    {
                        col_matches += 1;
                    }
                }

                let col_confidence = col_matches as f32 / overlap as f32;
                if col_confidence > 0.6 {
                    agreeing_cols += 1;
                }
                total_matches += col_matches;
            }

            if agreeing_cols >= 3 && total_matches > best_total_matches {
                best_total_matches = total_matches;
                best_dy = signed_dy;
                best_agreeing_cols = agreeing_cols;
            }
        }
    }

    if best_dy == 0 || best_agreeing_cols < 3 {
        return None;
    }

    let overlap = height - best_dy.unsigned_abs() as usize;
    let max_possible = overlap * sample_xs.len();
    let confidence = best_total_matches as f32 / max_possible as f32;

    if confidence > 0.7 {
        Some((best_dy, confidence))
    } else {
        None
    }
}

/// Select between CDP scroll hint and content-based scroll when both are
/// available during a wheel event.
pub fn select_wheel_trusted_scroll(
    cdp_dy: i16,
    input_scroll_dir: i32,
    content_scroll: Option<(i16, f32, bool, Option<f32>)>,
) -> Option<(i16, f32, &'static str, bool, Option<f32>)> {
    let cdp_dir = (cdp_dy as i32).signum();
    let cdp_direction_matches =
        input_scroll_dir == 0 || cdp_dir == 0 || input_scroll_dir == cdp_dir;
    if !cdp_direction_matches {
        return None;
    }

    if let Some((content_dy, content_confidence, content_direction_matches, content_min_confidence)) =
        content_scroll
    {
        let content_dir = (content_dy as i32).signum();
        let directions_compatible = content_direction_matches
            && (cdp_dir == 0 || content_dir == 0 || cdp_dir == content_dir);
        if directions_compatible {
            return Some((
                content_dy,
                content_confidence,
                "content",
                content_direction_matches,
                content_min_confidence,
            ));
        }
    }

    Some((cdp_dy, 1.0, "cdp", cdp_direction_matches, None))
}

/// Determine how far to search for scroll displacement based on CDP hints.
pub fn content_scroll_search_limit_px(cdp_scroll_dy_px: Option<i16>) -> usize {
    let cdp_abs = cdp_scroll_dy_px
        .map(|dy| (dy as i32).unsigned_abs() as usize)
        .unwrap_or(0);
    cdp_abs.clamp(256, CONTENT_SCROLL_SEARCH_MAX_PX)
}
