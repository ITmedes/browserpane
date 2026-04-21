//! Scroll tile partitioning and residual comparison: split tiles into
//! content/chrome regions and compute residual dirty set.

use crate::scroll::residual::{offset_tile_rect_for_emit, summarize_shifted_tile_diff};
use crate::scroll::{
    analyze_scroll_residual_emit_coords, can_emit_scroll_copy, has_scroll_region_split,
    is_content_tile_in_scroll_region, should_defer_scroll_repair,
};
use crate::tiles;

use super::frame_types::DetectedScrollFrame;

const STICKY_BAND_MIN_RATIO: f32 = 0.85;
const STICKY_BAND_MIN_TILES: usize = 4;
const STICKY_TOP_ROWS_MAX: u16 = 3;
const STICKY_RIGHT_COLS_MAX: u16 = 6;
const STICKY_BAND_MIN_SAVED_RATIO: f32 = 0.20;
const SMALL_EDGE_STRIP_MAX_ROWS: usize = 8;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct StickyBands {
    sticky_row_end: Option<u16>,
    sticky_col_start: Option<u16>,
}

impl StickyBands {
    fn is_active(self) -> bool {
        self.sticky_row_end.is_some() || self.sticky_col_start.is_some()
    }

    fn is_chrome(self, coord: tiles::TileCoord) -> bool {
        self.sticky_row_end
            .is_some_and(|row_end| coord.row < row_end)
            || self
                .sticky_col_start
                .is_some_and(|col_start| coord.col >= col_start)
    }
}

fn trim_sticky_bands(
    content_emit_coords: Vec<tiles::TileCoord>,
    mut chrome_emit_coords: Vec<tiles::TileCoord>,
    residual: &[tiles::TileCoord],
    saved_tiles: usize,
    grid: &tiles::TileGrid,
) -> (Vec<tiles::TileCoord>, Vec<tiles::TileCoord>, StickyBands) {
    if content_emit_coords.len() < STICKY_BAND_MIN_TILES * 4 || residual.is_empty() {
        return (
            content_emit_coords,
            chrome_emit_coords,
            StickyBands::default(),
        );
    }
    let saved_ratio = saved_tiles as f32 / content_emit_coords.len() as f32;
    if saved_ratio < STICKY_BAND_MIN_SAVED_RATIO {
        return (
            content_emit_coords,
            chrome_emit_coords,
            StickyBands::default(),
        );
    }

    let mut row_total = vec![0usize; grid.rows as usize];
    let mut row_residual = vec![0usize; grid.rows as usize];
    let mut col_total = vec![0usize; grid.cols as usize];
    let mut col_residual = vec![0usize; grid.cols as usize];
    let mut min_row = u16::MAX;
    let mut max_col = 0u16;

    for &coord in &content_emit_coords {
        min_row = min_row.min(coord.row);
        max_col = max_col.max(coord.col);
        row_total[coord.row as usize] += 1;
        col_total[coord.col as usize] += 1;
    }

    for &coord in residual {
        row_residual[coord.row as usize] += 1;
        col_residual[coord.col as usize] += 1;
    }

    if min_row == u16::MAX {
        return (
            content_emit_coords,
            chrome_emit_coords,
            StickyBands::default(),
        );
    }

    let mut sticky_top_rows = 0u16;
    let max_top_row = min_row
        .saturating_add(STICKY_TOP_ROWS_MAX.saturating_sub(1))
        .min(grid.rows.saturating_sub(1));
    for row in min_row..=max_top_row {
        let total = row_total[row as usize];
        if total < STICKY_BAND_MIN_TILES {
            break;
        }
        let ratio = row_residual[row as usize] as f32 / total as f32;
        if ratio < STICKY_BAND_MIN_RATIO {
            break;
        }
        sticky_top_rows = sticky_top_rows.saturating_add(1);
    }

    let mut sticky_right_cols = 0u16;
    let min_right_col = max_col.saturating_sub(STICKY_RIGHT_COLS_MAX.saturating_sub(1));
    for col in (min_right_col..=max_col).rev() {
        let total = col_total[col as usize];
        if total < STICKY_BAND_MIN_TILES {
            break;
        }
        let ratio = col_residual[col as usize] as f32 / total as f32;
        if ratio < STICKY_BAND_MIN_RATIO {
            break;
        }
        sticky_right_cols = sticky_right_cols.saturating_add(1);
    }

    if sticky_top_rows == 0 && sticky_right_cols == 0 {
        return (
            content_emit_coords,
            chrome_emit_coords,
            StickyBands::default(),
        );
    }

    let sticky_bands = StickyBands {
        sticky_row_end: (sticky_top_rows > 0).then_some(min_row.saturating_add(sticky_top_rows)),
        sticky_col_start: (sticky_right_cols > 0)
            .then_some(max_col.saturating_sub(sticky_right_cols.saturating_sub(1))),
    };
    let mut trimmed_content = Vec::with_capacity(content_emit_coords.len());
    for coord in content_emit_coords {
        if sticky_bands.is_chrome(coord) {
            chrome_emit_coords.push(coord);
        } else {
            trimmed_content.push(coord);
        }
    }

    (trimmed_content, chrome_emit_coords, sticky_bands)
}

/// Result of tile partitioning and residual comparison.
pub(crate) struct PartitionResult {
    pub content_emit_coords: Vec<tiles::TileCoord>,
    pub chrome_emit_coords: Vec<tiles::TileCoord>,
    pub residual: Vec<tiles::TileCoord>,
    pub exposed_strip_coords: Vec<tiles::TileCoord>,
    pub potential_tiles: usize,
    pub residual_tiles: usize,
    pub residual_ratio: f32,
    pub interior_ratio: f32,
    pub interior_residual_tiles: usize,
    pub edge_strip_residual_tiles: usize,
    pub small_edge_strip_residual_tiles: usize,
    pub small_edge_strip_residual_rows: usize,
    pub small_edge_strip_residual_area_px: usize,
    pub quantized_scroll_copy: bool,
    pub saved_tiles: usize,
    pub defer_scroll_repair: bool,
    pub scroll_row_shift: i32,
    pub scroll_dy: i16,
    pub split_region: bool,
    pub sticky_band_active: bool,
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
    let (mut content_emit_coords, mut chrome_emit_coords): (
        Vec<tiles::TileCoord>,
        Vec<tiles::TileCoord>,
    ) = if have_split {
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

    let initial_analysis = analyze_scroll_residual_emit_coords(
        rgba,
        prev,
        stride,
        grid,
        grid_offset_y,
        scroll_dy,
        &content_emit_coords,
    );

    let saved_tiles = content_emit_coords
        .len()
        .saturating_sub(initial_analysis.residual.len());
    let sticky_bands;
    (content_emit_coords, chrome_emit_coords, sticky_bands) = trim_sticky_bands(
        content_emit_coords,
        chrome_emit_coords,
        &initial_analysis.residual,
        saved_tiles,
        grid,
    );
    let emit_area_px = |coord: tiles::TileCoord| {
        let rect = offset_tile_rect_for_emit(coord, grid, grid_offset_y);
        rect.w as usize * rect.h as usize
    };
    let chrome_residual_area_px = initial_analysis
        .residual
        .iter()
        .filter(|coord| sticky_bands.is_chrome(**coord))
        .map(|coord| emit_area_px(*coord))
        .sum::<usize>();
    let chrome_exposed_strip_area_px = initial_analysis
        .exposed_strip
        .iter()
        .filter(|coord| sticky_bands.is_chrome(**coord))
        .map(|coord| emit_area_px(*coord))
        .sum::<usize>();
    let residual: Vec<_> = initial_analysis
        .residual
        .into_iter()
        .filter(|coord| !sticky_bands.is_chrome(*coord))
        .collect();
    let exposed_strip_coords: Vec<_> = initial_analysis
        .exposed_strip
        .into_iter()
        .filter(|coord| !sticky_bands.is_chrome(*coord))
        .collect();
    let residual_area_px = initial_analysis
        .residual_area_px
        .saturating_sub(chrome_residual_area_px);
    let exposed_strip_area_px = initial_analysis
        .exposed_strip_area_px
        .saturating_sub(chrome_exposed_strip_area_px);

    let potential_tiles = content_emit_coords.len();
    let residual_tiles = residual.len();
    let residual_ratio = if potential_tiles == 0 {
        1.0
    } else {
        residual_tiles as f32 / potential_tiles as f32
    };

    let exposed_strip_count = exposed_strip_coords.len();
    let interior_residual = residual_tiles.saturating_sub(exposed_strip_count);
    let potential_area_px = content_emit_coords
        .iter()
        .map(|coord| emit_area_px(*coord))
        .sum::<usize>();
    let interior_area_px = potential_area_px.saturating_sub(exposed_strip_area_px);
    let interior_ratio = if interior_area_px == 0 {
        0.0
    } else {
        residual_area_px.saturating_sub(exposed_strip_area_px) as f32 / interior_area_px as f32
    };
    let mut edge_strip_residual_tiles = 0usize;
    let mut small_edge_strip_residual_tiles = 0usize;
    let mut small_edge_strip_residual_rows = 0usize;
    let mut small_edge_strip_residual_area_px = 0usize;
    for &coord in &residual {
        if exposed_strip_coords.contains(&coord) {
            continue;
        }
        let rect = offset_tile_rect_for_emit(coord, grid, grid_offset_y);
        if rect.w == 0 || rect.h == 0 {
            continue;
        }
        let diff = summarize_shifted_tile_diff(
            rgba,
            prev,
            stride,
            grid.screen_h as usize,
            &rect,
            scroll_dy,
        );
        if !diff.is_edge_strip(rect.h as usize) {
            continue;
        }
        edge_strip_residual_tiles += 1;
        if diff.changed_rows <= SMALL_EDGE_STRIP_MAX_ROWS {
            small_edge_strip_residual_tiles += 1;
            small_edge_strip_residual_rows += diff.changed_rows;
            small_edge_strip_residual_area_px += diff.changed_area_px(rect.w as usize);
        }
    }
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
        exposed_strip_coords,
        potential_tiles,
        residual_tiles,
        residual_ratio,
        interior_ratio,
        interior_residual_tiles: interior_residual,
        edge_strip_residual_tiles,
        small_edge_strip_residual_tiles,
        small_edge_strip_residual_rows,
        small_edge_strip_residual_area_px,
        quantized_scroll_copy,
        saved_tiles,
        defer_scroll_repair,
        scroll_row_shift,
        scroll_dy,
        split_region: have_split,
        sticky_band_active: sticky_bands.is_active(),
    }
}
