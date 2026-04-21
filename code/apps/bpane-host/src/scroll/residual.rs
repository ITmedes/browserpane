//! Tile-level scroll residual analysis: which tiles actually changed after
//! accounting for the detected vertical displacement.

use crate::tiles;

pub struct ScrollResidualCoords {
    pub residual: Vec<tiles::TileCoord>,
    pub exposed_strip: Vec<tiles::TileCoord>,
}

fn band_matches_shifted_prev(
    current: &[u8],
    previous: &[u8],
    stride: usize,
    y: usize,
    h: usize,
    x_bytes: usize,
    row_bytes: usize,
    scroll_dy: i16,
) -> bool {
    let dy = scroll_dy as i32;
    for row in 0..h {
        let cy = y as i32 + row as i32;
        let py = cy + dy;
        if py < 0 {
            return false;
        }
        let curr_off = cy as usize * stride + x_bytes;
        let prev_off = py as usize * stride + x_bytes;
        let curr_end = curr_off + row_bytes;
        let prev_end = prev_off + row_bytes;
        if curr_end > current.len() || prev_end > previous.len() {
            return false;
        }
        if current[curr_off..curr_end] != previous[prev_off..prev_end] {
            return false;
        }
    }
    true
}

fn contiguous_row_span(row_coords: &[tiles::TileCoord]) -> Option<(u16, u16)> {
    let first = row_coords.first()?.col;
    let mut expected = first;
    let mut last = first;
    for coord in row_coords {
        if coord.col != expected {
            return None;
        }
        last = coord.col;
        expected = expected.saturating_add(1);
    }
    Some((first, last))
}

fn analyze_scroll_residual_row_bands(
    current: &[u8],
    previous: &[u8],
    stride: usize,
    grid: &tiles::TileGrid,
    scroll_dy: i16,
    emit_coords: &[tiles::TileCoord],
) -> ScrollResidualCoords {
    let dy = scroll_dy as i32;
    let screen_h = grid.screen_h as i32;
    let ts = grid.tile_size;
    let mut residual = Vec::with_capacity(emit_coords.len() / 2);
    let mut exposed_strip = Vec::with_capacity((emit_coords.len() / 8).max(1));
    let mut index = 0usize;

    while index < emit_coords.len() {
        let row = emit_coords[index].row;
        let row_start = index;
        while index < emit_coords.len() && emit_coords[index].row == row {
            index += 1;
        }
        let row_coords = &emit_coords[row_start..index];
        let y = row as usize * ts as usize;
        let h = usize::from(ts.min(grid.screen_h.saturating_sub(row * ts)));
        if h == 0 {
            continue;
        }

        let shifted_top = y as i32 + dy;
        let shifted_bottom = y as i32 + h as i32 - 1 + dy;
        if shifted_top < 0 || shifted_bottom >= screen_h {
            exposed_strip.extend_from_slice(row_coords);
            residual.extend_from_slice(row_coords);
            continue;
        }

        let row_matches = contiguous_row_span(row_coords).is_some_and(|(first_col, last_col)| {
            let x = usize::from(first_col) * usize::from(ts);
            let last_x = usize::from(last_col) * usize::from(ts);
            let last_tile_w =
                usize::from(ts.min(grid.screen_w.saturating_sub(last_col.saturating_mul(ts))));
            let row_bytes = last_x
                .saturating_add(last_tile_w)
                .saturating_sub(x)
                .saturating_mul(4);
            row_bytes > 0
                && band_matches_shifted_prev(
                    current,
                    previous,
                    stride,
                    y,
                    h,
                    x * 4,
                    row_bytes,
                    scroll_dy,
                )
        });
        if row_matches {
            continue;
        }

        for &coord in row_coords {
            let x = coord.col * ts;
            let w = ts.min(grid.screen_w.saturating_sub(x));
            let rect = tiles::Rect::new(x, row * ts, w, h as u16);
            if !tile_matches_shifted_prev(
                current,
                previous,
                stride,
                grid.screen_h as usize,
                &rect,
                scroll_dy,
            ) {
                residual.push(coord);
            }
        }
    }

    ScrollResidualCoords {
        residual,
        exposed_strip,
    }
}

/// Compute the tile extraction rect used by the emitter when a vertical
/// grid offset is active.
pub fn offset_tile_rect_for_emit(
    coord: tiles::TileCoord,
    grid: &tiles::TileGrid,
    offset_y: u16,
) -> tiles::Rect {
    let ts = grid.tile_size;
    let x = coord.col as u16 * ts;
    let raw_y = coord.row as i32 * ts as i32 - offset_y as i32;
    let fb_y = raw_y.max(0) as u16;
    let fb_end_y = ((raw_y + ts as i32).min(grid.screen_h as i32)).max(0) as u16;
    let h = fb_end_y.saturating_sub(fb_y);
    let w = ts.min(grid.screen_w.saturating_sub(x));
    if w == 0 || h == 0 {
        return tiles::Rect::new(0, 0, 0, 0);
    }
    tiles::Rect::new(x, fb_y, w, h)
}

/// Returns true when current tile pixels equal previous shifted by scroll dy.
/// A mismatch or out-of-bounds mapping marks the tile as residual-dirty.
pub fn tile_matches_shifted_prev(
    current: &[u8],
    previous: &[u8],
    stride: usize,
    screen_h: usize,
    rect: &tiles::Rect,
    scroll_dy: i16,
) -> bool {
    if rect.w == 0 || rect.h == 0 {
        return true;
    }
    let dy = scroll_dy as i32;
    let x_bytes = rect.x as usize * 4;
    let row_bytes = rect.w as usize * 4;

    for row in 0..rect.h as usize {
        let cy = rect.y as i32 + row as i32;
        let py = cy + dy;
        if py < 0 || py >= screen_h as i32 {
            return false;
        }
        let curr_off = cy as usize * stride + x_bytes;
        let prev_off = py as usize * stride + x_bytes;
        let curr_end = curr_off + row_bytes;
        let prev_end = prev_off + row_bytes;
        if curr_end > current.len() || prev_end > previous.len() {
            return false;
        }
        if current[curr_off..curr_end] != previous[prev_off..prev_end] {
            return false;
        }
    }
    true
}

/// Build the residual dirty set for a trusted vertical scroll frame.
/// Compares current pixels against previous shifted by `scroll_dy`;
/// only mismatches are included.
pub fn analyze_scroll_residual_emit_coords(
    current: &[u8],
    previous: &[u8],
    stride: usize,
    grid: &tiles::TileGrid,
    grid_offset_y: u16,
    scroll_dy: i16,
    emit_coords: &[tiles::TileCoord],
) -> ScrollResidualCoords {
    if scroll_dy == 0 || emit_coords.is_empty() {
        return ScrollResidualCoords {
            residual: emit_coords.to_vec(),
            exposed_strip: Vec::new(),
        };
    }
    if grid_offset_y == 0
        && grid.tile_size > 0
        && i32::from(scroll_dy).unsigned_abs() % u32::from(grid.tile_size) == 0
    {
        return analyze_scroll_residual_row_bands(
            current,
            previous,
            stride,
            grid,
            scroll_dy,
            emit_coords,
        );
    }

    let dy = scroll_dy as i32;
    let screen_h = grid.screen_h as i32;
    let mut residual = Vec::with_capacity(emit_coords.len() / 2);
    let mut exposed_strip = Vec::with_capacity((emit_coords.len() / 8).max(1));

    for &coord in emit_coords {
        let rect = offset_tile_rect_for_emit(coord, grid, grid_offset_y);
        if rect.w == 0 || rect.h == 0 {
            continue;
        }
        let shifted_top = rect.y as i32 + dy;
        let shifted_bottom = rect.y as i32 + rect.h as i32 - 1 + dy;
        if shifted_top < 0 || shifted_bottom >= screen_h {
            exposed_strip.push(coord);
            residual.push(coord);
            continue;
        }
        if !tile_matches_shifted_prev(
            current,
            previous,
            stride,
            grid.screen_h as usize,
            &rect,
            scroll_dy,
        ) {
            residual.push(coord);
        }
    }

    ScrollResidualCoords {
        residual,
        exposed_strip,
    }
}

pub fn build_scroll_residual_emit_coords(
    current: &[u8],
    previous: &[u8],
    stride: usize,
    grid: &tiles::TileGrid,
    grid_offset_y: u16,
    scroll_dy: i16,
    emit_coords: &[tiles::TileCoord],
) -> Vec<tiles::TileCoord> {
    analyze_scroll_residual_emit_coords(
        current,
        previous,
        stride,
        grid,
        grid_offset_y,
        scroll_dy,
        emit_coords,
    )
    .residual
}

/// Build the exposed-strip dirty set for a vertical scroll copy.
/// Returns only tiles whose shifted position falls outside the screen.
pub fn build_scroll_exposed_strip_emit_coords(
    grid: &tiles::TileGrid,
    grid_offset_y: u16,
    scroll_dy: i16,
    emit_coords: &[tiles::TileCoord],
) -> Vec<tiles::TileCoord> {
    if scroll_dy == 0 || emit_coords.is_empty() {
        return Vec::new();
    }
    let dy = scroll_dy as i32;
    let screen_h = grid.screen_h as i32;
    let mut out = Vec::with_capacity((emit_coords.len() / 8).max(1));
    for &coord in emit_coords {
        let rect = offset_tile_rect_for_emit(coord, grid, grid_offset_y);
        if rect.w == 0 || rect.h == 0 {
            continue;
        }
        let shifted_top = rect.y as i32 + dy;
        let shifted_bottom = rect.y as i32 + rect.h as i32 - 1 + dy;
        if shifted_top < 0 || shifted_bottom >= screen_h {
            out.push(coord);
        }
    }
    out
}
