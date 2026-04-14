//! Scroll tile partitioning and residual comparison: split tiles into
//! content/chrome regions and compute residual dirty set.

use std::collections::HashSet;

use crate::scroll::{
    build_scroll_exposed_strip_emit_coords, build_scroll_residual_emit_coords,
    can_emit_scroll_copy, has_scroll_region_split, is_content_tile_in_scroll_region,
    should_defer_scroll_repair,
};
use crate::tiles;

use super::frame_types::DetectedScrollFrame;

/// Result of tile partitioning and residual comparison.
pub(crate) struct PartitionResult {
    pub content_emit_coords: Vec<tiles::TileCoord>,
    pub chrome_emit_coords: Vec<tiles::TileCoord>,
    pub residual: Vec<tiles::TileCoord>,
    pub potential_tiles: usize,
    pub residual_tiles: usize,
    pub residual_ratio: f32,
    pub interior_ratio: f32,
    pub quantized_scroll_copy: bool,
    pub saved_tiles: usize,
    pub defer_scroll_repair: bool,
    pub scroll_row_shift: i32,
    pub srt_for_split: u16,
    pub srb_for_split: u16,
    pub scroll_dy: i16,
}

/// Partition tiles into content/chrome, compute residual, and return
/// comparison metrics. Pure computation; does not mutate thread state.
#[allow(clippy::too_many_arguments)]
pub(crate) fn partition_and_compare(
    rgba: &[u8],
    prev: &[u8],
    stride: usize,
    grid: &tiles::TileGrid,
    grid_offset_y: u16,
    tile_size: u16,
    scroll_copy_quantum_px: u16,
    scroll_dy: i16,
    full_emit_coords: &[tiles::TileCoord],
    detected_scroll_frame: &DetectedScrollFrame,
    _last_scroll_region_top: u16,
    screen_h: u16,
    _last_scroll_region_right: u16,
    screen_w: u16,
) -> PartitionResult {
    let scroll_row_shift = detected_scroll_frame.row_shift;
    let srt_for_split = detected_scroll_frame.region_top;
    let srb_for_split = detected_scroll_frame.region_bottom;
    let srr_for_split = detected_scroll_frame.region_right;
    let ts = tile_size;

    let have_split = has_scroll_region_split(
        srt_for_split,
        srb_for_split,
        srr_for_split,
        screen_h,
        screen_w,
    );
    let (content_emit_coords, chrome_emit_coords): (Vec<tiles::TileCoord>, Vec<tiles::TileCoord>) =
        if have_split {
            full_emit_coords.iter().partition(|coord| {
                is_content_tile_in_scroll_region(
                    **coord,
                    ts,
                    srt_for_split,
                    srb_for_split,
                    srr_for_split,
                )
            })
        } else {
            (full_emit_coords.to_vec(), Vec::new())
        };

    let residual = build_scroll_residual_emit_coords(
        rgba,
        prev,
        stride,
        grid,
        grid_offset_y,
        scroll_dy,
        &content_emit_coords,
    );

    let potential_tiles = content_emit_coords.len();
    let residual_tiles = residual.len();
    let residual_ratio = if potential_tiles == 0 {
        1.0
    } else {
        residual_tiles as f32 / potential_tiles as f32
    };

    let exposed_tiles: HashSet<_> = build_scroll_exposed_strip_emit_coords(
        grid,
        grid_offset_y,
        scroll_dy,
        &content_emit_coords,
    )
    .into_iter()
    .collect();
    let interior_residual = residual
        .iter()
        .filter(|c| !exposed_tiles.contains(c))
        .count();
    let interior_total = potential_tiles.saturating_sub(exposed_tiles.len());
    let interior_ratio = if interior_total == 0 {
        0.0
    } else {
        interior_residual as f32 / interior_total as f32
    };
    let quantized_scroll_copy = can_emit_scroll_copy(scroll_dy, scroll_copy_quantum_px, tile_size);
    let saved_tiles = potential_tiles.saturating_sub(residual_tiles);
    let defer_scroll_repair = should_defer_scroll_repair(
        quantized_scroll_copy,
        interior_ratio,
        saved_tiles,
        potential_tiles,
        scroll_row_shift,
    );

    PartitionResult {
        content_emit_coords,
        chrome_emit_coords,
        residual,
        potential_tiles,
        residual_tiles,
        residual_ratio,
        interior_ratio,
        quantized_scroll_copy,
        saved_tiles,
        defer_scroll_repair,
        scroll_row_shift,
        srt_for_split,
        srb_for_split,
        scroll_dy,
    }
}
