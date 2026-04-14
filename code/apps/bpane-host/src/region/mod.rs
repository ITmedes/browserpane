//! Geometry helpers for capture regions, tile bounds, and coordinate scaling.

#[cfg(test)]
mod tests;

use crate::capture::ffmpeg::CaptureRegion;
use crate::tiles;

pub const MIN_EDITABLE_HINT_WIDTH_PX: u32 = 2;
pub const MIN_EDITABLE_HINT_HEIGHT_PX: u32 = 2;

// ── Coordinate scaling ──────────────────────────────────────────────

/// Convert CSS logical pixels to screen (framebuffer) pixels.
/// `scale_milli` is the device-pixel-ratio × 1000 (e.g. 2000 for 2×).
pub fn scale_css_px_to_screen_px(css_px: i64, scale_milli: u16) -> i64 {
    let scale = i64::from(scale_milli.max(1));
    let scaled = css_px.saturating_mul(scale);
    if scaled >= 0 {
        scaled.saturating_add(500) / 1000
    } else {
        scaled.saturating_sub(500) / 1000
    }
}

// ── Region validation ───────────────────────────────────────────────

pub fn region_meets_video_minimum(
    w: u32,
    h: u32,
    screen_w: u32,
    screen_h: u32,
    min_w: u32,
    min_h: u32,
    min_area_ratio: f32,
) -> bool {
    if w < min_w || h < min_h || screen_w == 0 || screen_h == 0 {
        return false;
    }
    let area = (w as u64).saturating_mul(h as u64);
    let screen_area = (screen_w as u64).saturating_mul(screen_h as u64).max(1);
    let ratio = area as f32 / screen_area as f32;
    ratio >= min_area_ratio
}

pub fn region_meets_editable_minimum(w: u32, h: u32) -> bool {
    w >= MIN_EDITABLE_HINT_WIDTH_PX && h >= MIN_EDITABLE_HINT_HEIGHT_PX
}

pub fn clamp_region_to_screen(
    region: CaptureRegion,
    screen_w: u32,
    screen_h: u32,
) -> Option<CaptureRegion> {
    if screen_w < 2 || screen_h < 2 {
        return None;
    }
    let x0 = region.x.min(screen_w.saturating_sub(1));
    let y0 = region.y.min(screen_h.saturating_sub(1));
    let x1 = region.x.saturating_add(region.w).min(screen_w);
    let y1 = region.y.saturating_add(region.h).min(screen_h);
    if x1 <= x0 || y1 <= y0 {
        return None;
    }
    let mut w = x1 - x0;
    let mut h = y1 - y0;
    if w & 1 == 1 {
        w = w.saturating_sub(1);
    }
    if h & 1 == 1 {
        h = h.saturating_sub(1);
    }
    if w < 2 || h < 2 {
        return None;
    }
    Some(CaptureRegion { x: x0, y: y0, w, h })
}

pub fn point_in_capture_region(x: u16, y: u16, region: CaptureRegion) -> bool {
    let px = x as u32;
    let py = y as u32;
    let x1 = region.x.saturating_add(region.w);
    let y1 = region.y.saturating_add(region.h);
    px >= region.x && px < x1 && py >= region.y && py < y1
}

// ── Tile bounds ─────────────────────────────────────────────────────

pub fn capture_region_tile_bounds(
    region: CaptureRegion,
    tile_size: u16,
    cols: u16,
    rows: u16,
) -> (u16, u16, u16, u16) {
    if tile_size == 0 || cols == 0 || rows == 0 || region.w == 0 || region.h == 0 {
        return (0, 0, 0, 0);
    }
    let ts = tile_size as u32;
    let max_col = cols.saturating_sub(1) as u32;
    let max_row = rows.saturating_sub(1) as u32;
    let x1 = region.x.saturating_add(region.w.saturating_sub(1));
    let y1 = region.y.saturating_add(region.h.saturating_sub(1));
    let min_col = (region.x / ts).min(max_col) as u16;
    let min_row = (region.y / ts).min(max_row) as u16;
    let max_col = (x1 / ts).min(max_col) as u16;
    let max_row = (y1 / ts).min(max_row) as u16;
    (min_col, min_row, max_col, max_row)
}

pub fn tile_bounds_capture_region(
    bounds: (u16, u16, u16, u16),
    tile_size: u16,
    screen_w: u32,
    screen_h: u32,
) -> Option<CaptureRegion> {
    if tile_size == 0 || screen_w == 0 || screen_h == 0 {
        return None;
    }
    let ts = u32::from(tile_size);
    let (min_col, min_row, max_col, max_row) = bounds;
    let x = u32::from(min_col).saturating_mul(ts);
    let y = u32::from(min_row).saturating_mul(ts);
    let x1 = u32::from(max_col.saturating_add(1))
        .saturating_mul(ts)
        .min(screen_w);
    let y1 = u32::from(max_row.saturating_add(1))
        .saturating_mul(ts)
        .min(screen_h);
    if x1 <= x || y1 <= y {
        return None;
    }
    clamp_region_to_screen(
        CaptureRegion {
            x,
            y,
            w: x1 - x,
            h: y1 - y,
        },
        screen_w,
        screen_h,
    )
}

pub fn align_capture_region_to_tiles(
    region: CaptureRegion,
    tile_size: u16,
    cols: u16,
    rows: u16,
    screen_w: u32,
    screen_h: u32,
) -> Option<CaptureRegion> {
    tile_bounds_capture_region(
        capture_region_tile_bounds(region, tile_size, cols, rows),
        tile_size,
        screen_w,
        screen_h,
    )
}

pub fn align_capture_region_to_tiles_with_margin(
    region: CaptureRegion,
    tile_size: u16,
    cols: u16,
    rows: u16,
    screen_w: u32,
    screen_h: u32,
    margin: u16,
) -> Option<CaptureRegion> {
    let bounds = capture_region_tile_bounds(region, tile_size, cols, rows);
    let expanded = expand_tile_bounds(bounds, margin, cols, rows);
    tile_bounds_capture_region(expanded, tile_size, screen_w, screen_h)
}

pub fn expand_tile_bounds(
    bounds: (u16, u16, u16, u16),
    margin: u16,
    cols: u16,
    rows: u16,
) -> (u16, u16, u16, u16) {
    if cols == 0 || rows == 0 {
        return (0, 0, 0, 0);
    }
    let (min_col, min_row, max_col, max_row) = bounds;
    let max_grid_col = cols.saturating_sub(1);
    let max_grid_row = rows.saturating_sub(1);
    (
        min_col.saturating_sub(margin),
        min_row.saturating_sub(margin),
        max_col.saturating_add(margin).min(max_grid_col),
        max_row.saturating_add(margin).min(max_grid_row),
    )
}

pub fn extend_dirty_with_tile_bounds(
    dirty: &mut Vec<tiles::TileCoord>,
    bounds: (u16, u16, u16, u16),
) {
    let (min_col, min_row, max_col, max_row) = bounds;
    for row in min_row..=max_row {
        for col in min_col..=max_col {
            let coord = tiles::TileCoord::new(col, row);
            if !dirty.contains(&coord) {
                dirty.push(coord);
            }
        }
    }
}

// ── Hashing ─────────────────────────────────────────────────────────

/// Hash a tile region directly from the frame buffer without allocating.
pub fn hash_tile_region(
    frame: &[u8],
    stride: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) -> u64 {
    use xxhash_rust::xxh3::xxh3_64;
    let row_bytes = w * 4;
    if w * 4 == stride {
        let start = y * stride + x * 4;
        let end = start + h * stride;
        if end <= frame.len() {
            return xxh3_64(&frame[start..end]);
        }
    }
    let mut buf = Vec::with_capacity(row_bytes * h);
    for row in 0..h {
        let start = (y + row) * stride + x * 4;
        let end = start + row_bytes;
        if end <= frame.len() {
            buf.extend_from_slice(&frame[start..end]);
        }
    }
    xxh3_64(&buf)
}

// ── CDP text input helper ───────────────────────────────────────────

/// Build a CDP `Input.insertText` payload for non-ASCII printable chars
/// that bypass the keyboard layout entirely.
pub fn cdp_insert_text_payload(modifiers: u8, key_char: u32) -> Option<String> {
    if key_char < 0x80 || crate::input::keyboard::should_use_physical(modifiers, key_char) {
        return None;
    }
    char::from_u32(key_char)
        .filter(|ch| !ch.is_control())
        .map(|ch| ch.to_string())
}
