//! Heuristic decision functions for scroll behaviour: when to emit
//! ScrollCopy, when to defer repair, capture cadence, quantisation.

use super::{
    SCROLL_DEFER_REPAIR_MAX_INTERIOR_RATIO, SCROLL_DEFER_REPAIR_MAX_ROW_SHIFT,
    SCROLL_DEFER_REPAIR_MIN_SAVED_RATIO, SCROLL_RESIDUAL_FULL_REPAINT_RATIO_DEFAULT,
};
use crate::tiles;
use std::time::Duration;

/// Skip ScrollCopy when residual analysis says the frame is a full repaint.
pub fn should_emit_scroll_copy(
    scroll_residual_fallback_full: bool,
    scroll_saved_tiles: Option<usize>,
) -> bool {
    if scroll_residual_fallback_full {
        return false;
    }
    scroll_saved_tiles.unwrap_or(1) > 0
}

/// Choose faster capture interval during active scrolling.
pub fn select_capture_frame_interval(
    base_frame_interval: Duration,
    scroll_active_frame_interval: Duration,
    scroll_active_capture_frames_remaining: u8,
) -> Duration {
    if scroll_active_capture_frames_remaining > 0 {
        scroll_active_frame_interval.min(base_frame_interval)
    } else {
        base_frame_interval
    }
}

/// Refresh the fast-capture window counter based on scroll activity.
pub fn next_scroll_active_capture_frames(
    scroll_active_capture_frames_remaining: u8,
    scroll_active_capture_frames: u8,
    pending_scrolls: i32,
    strong_scroll_observed: bool,
    cdp_scroll_observed: bool,
) -> u8 {
    if pending_scrolls > 0 || strong_scroll_observed || cdp_scroll_observed {
        scroll_active_capture_frames
    } else {
        scroll_active_capture_frames_remaining.saturating_sub(1)
    }
}

/// Defer repair for moderate residual with small row shifts.
pub fn should_defer_scroll_repair(
    quantized_scroll_copy: bool,
    interior_ratio: f32,
    saved_tiles: usize,
    potential_tiles: usize,
    row_shift: i32,
) -> bool {
    if !quantized_scroll_copy || potential_tiles == 0 {
        return false;
    }
    if interior_ratio <= SCROLL_RESIDUAL_FULL_REPAINT_RATIO_DEFAULT
        || interior_ratio > SCROLL_DEFER_REPAIR_MAX_INTERIOR_RATIO
    {
        return false;
    }
    let saved_ratio = saved_tiles as f32 / potential_tiles as f32;
    row_shift.abs() <= SCROLL_DEFER_REPAIR_MAX_ROW_SHIFT
        && saved_ratio >= SCROLL_DEFER_REPAIR_MIN_SAVED_RATIO
}

/// Check if a scroll delta is quantized to the given pixel quantum.
pub fn is_scroll_delta_quantized(scroll_dy: i16, quantum_px: u16) -> bool {
    if quantum_px == 0 {
        return true;
    }
    let dy = i32::from(scroll_dy).unsigned_abs();
    dy % u32::from(quantum_px) == 0
}

/// Can we emit a ScrollCopy for this delta? Requires non-zero, quantized,
/// and a whole number of tile rows.
pub fn can_emit_scroll_copy(scroll_dy: i16, quantum_px: u16, tile_size: u16) -> bool {
    if tile_size == 0 {
        return false;
    }
    let dy = i32::from(scroll_dy).unsigned_abs();
    dy != 0 && is_scroll_delta_quantized(scroll_dy, quantum_px) && dy % u32::from(tile_size) == 0
}

/// Detect whether the scroll region covers only part of the viewport.
pub fn has_scroll_region_split(
    region_top: u16,
    region_bottom: u16,
    region_right: u16,
    screen_h: u16,
    screen_w: u16,
) -> bool {
    region_top > 0 || region_bottom < screen_h || region_right < screen_w
}

/// Check if a tile fits fully inside the scroll region.
pub fn is_content_tile_in_scroll_region(
    coord: tiles::TileCoord,
    tile_size: u16,
    region_top: u16,
    region_bottom: u16,
    region_right: u16,
) -> bool {
    let tile_top = coord.row * tile_size;
    let tile_left = coord.col * tile_size;
    tile_top >= region_top
        && tile_top.saturating_add(tile_size) <= region_bottom
        && tile_left.saturating_add(tile_size) <= region_right
}
