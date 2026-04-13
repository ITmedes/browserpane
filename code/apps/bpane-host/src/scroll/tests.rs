use super::*;
use crate::tiles;

// ── Residual tests ──────────────────────────────────────────────────

#[test]
fn tile_matches_shifted_prev_detects_exposed_edge() {
    let width = 8usize;
    let height = 8usize;
    let stride = width * 4;
    let mut prev = vec![0u8; stride * height];
    for y in 0..height {
        for x in 0..width {
            let off = y * stride + x * 4;
            prev[off] = y as u8;
            prev[off + 1] = y as u8;
            prev[off + 2] = y as u8;
            prev[off + 3] = 255;
        }
    }

    // current[y] = prev[y+1] for y in 0..height-1 (dy = +1, content moved up)
    let mut curr = vec![0u8; stride * height];
    for y in 0..(height - 1) {
        let dst = y * stride;
        let src = (y + 1) * stride;
        curr[dst..dst + stride].copy_from_slice(&prev[src..src + stride]);
    }
    for x in 0..width {
        let off = (height - 1) * stride + x * 4;
        curr[off] = 0xEE;
        curr[off + 1] = 0xEE;
        curr[off + 2] = 0xEE;
        curr[off + 3] = 255;
    }

    let interior = tiles::Rect::new(0, 0, width as u16, (height - 1) as u16);
    assert!(tile_matches_shifted_prev(
        &curr, &prev, stride, height, &interior, 1
    ));

    let exposed = tiles::Rect::new(0, (height - 1) as u16, width as u16, 1);
    assert!(!tile_matches_shifted_prev(
        &curr, &prev, stride, height, &exposed, 1
    ));
}

#[test]
fn scroll_residual_emit_marks_exposed_rows() {
    let grid = tiles::TileGrid::new(128, 128, 64);
    let stride = 128usize * 4;
    let prev = vec![0u8; stride * 128usize];
    let curr = vec![0u8; stride * 128usize];
    let emit_coords: Vec<tiles::TileCoord> = (0..grid.rows)
        .flat_map(|r| (0..grid.cols).map(move |c| tiles::TileCoord::new(c, r)))
        .collect();

    let down =
        build_scroll_residual_emit_coords(&curr, &prev, stride, &grid, 0, 1, &emit_coords);
    assert!(down.contains(&tiles::TileCoord::new(0, 1)));
    assert!(down.contains(&tiles::TileCoord::new(1, 1)));
    assert!(!down.contains(&tiles::TileCoord::new(0, 0)));
    assert!(!down.contains(&tiles::TileCoord::new(1, 0)));

    let up =
        build_scroll_residual_emit_coords(&curr, &prev, stride, &grid, 0, -1, &emit_coords);
    assert!(up.contains(&tiles::TileCoord::new(0, 0)));
    assert!(up.contains(&tiles::TileCoord::new(1, 0)));
    assert!(!up.contains(&tiles::TileCoord::new(0, 1)));
    assert!(!up.contains(&tiles::TileCoord::new(1, 1)));
}

#[test]
fn scroll_exposed_strip_marks_only_exposed_rows() {
    let grid = tiles::TileGrid::new(128, 128, 64);
    let emit_coords: Vec<tiles::TileCoord> = (0..grid.rows)
        .flat_map(|r| (0..grid.cols).map(move |c| tiles::TileCoord::new(c, r)))
        .collect();

    let down = build_scroll_exposed_strip_emit_coords(&grid, 0, 1, &emit_coords);
    assert!(down.contains(&tiles::TileCoord::new(0, 1)));
    assert!(down.contains(&tiles::TileCoord::new(1, 1)));
    assert!(!down.contains(&tiles::TileCoord::new(0, 0)));
    assert!(!down.contains(&tiles::TileCoord::new(1, 0)));

    let up = build_scroll_exposed_strip_emit_coords(&grid, 0, -1, &emit_coords);
    assert!(up.contains(&tiles::TileCoord::new(0, 0)));
    assert!(up.contains(&tiles::TileCoord::new(1, 0)));
    assert!(!up.contains(&tiles::TileCoord::new(0, 1)));
    assert!(!up.contains(&tiles::TileCoord::new(1, 1)));
}

// ── Policy tests ────────────────────────────────────────────────────

#[test]
fn scroll_copy_policy_skips_full_repaints() {
    assert!(!should_emit_scroll_copy(true, Some(12)));
    assert!(!should_emit_scroll_copy(false, Some(0)));
    assert!(should_emit_scroll_copy(false, Some(1)));
    assert!(should_emit_scroll_copy(false, None));
}

#[test]
fn moderate_residual_can_defer_repair_for_small_row_shifts() {
    assert!(should_defer_scroll_repair(true, 0.76, 48, 160, 2));
    assert!(should_defer_scroll_repair(true, 0.71, 32, 120, 1));
}

#[test]
fn severe_or_low_value_residual_still_forces_full_repaint() {
    assert!(!should_defer_scroll_repair(true, 0.90, 48, 160, 2));
    assert!(!should_defer_scroll_repair(true, 0.76, 12, 160, 2));
    assert!(!should_defer_scroll_repair(true, 0.76, 48, 160, 3));
    assert!(!should_defer_scroll_repair(false, 0.76, 48, 160, 2));
}

#[test]
fn scroll_activity_uses_faster_capture_interval() {
    let base = std::time::Duration::from_millis(100);
    let active = std::time::Duration::from_millis(50);
    assert_eq!(select_capture_frame_interval(base, active, 0), base);
    assert_eq!(select_capture_frame_interval(base, active, 3), active);
}

#[test]
fn scroll_activity_refreshes_fast_capture_window() {
    assert_eq!(next_scroll_active_capture_frames(0, 8, 1, false, false), 8);
    assert_eq!(next_scroll_active_capture_frames(4, 8, 0, true, false), 8);
    assert_eq!(next_scroll_active_capture_frames(4, 8, 0, false, true), 8);
    assert_eq!(next_scroll_active_capture_frames(4, 8, 0, false, false), 3);
    assert_eq!(next_scroll_active_capture_frames(0, 8, 0, false, false), 0);
}

#[test]
fn scroll_delta_quantization_checks_screen_pixels() {
    assert!(is_scroll_delta_quantized(64, 64));
    assert!(is_scroll_delta_quantized(-128, 64));
    assert!(!is_scroll_delta_quantized(96, 64));
    assert!(is_scroll_delta_quantized(96, 0));
}

#[test]
fn scroll_copy_requires_whole_tile_moves() {
    assert!(can_emit_scroll_copy(64, 64, 64));
    assert!(can_emit_scroll_copy(-128, 64, 64));
    assert!(!can_emit_scroll_copy(32, 32, 64));
    assert!(!can_emit_scroll_copy(96, 64, 64));
    assert!(!can_emit_scroll_copy(0, 64, 64));
}

#[test]
fn scroll_region_split_detects_partial_viewports() {
    assert!(!has_scroll_region_split(0, 768, 1280, 768, 1280));
    assert!(has_scroll_region_split(84, 768, 1280, 768, 1280));
    assert!(has_scroll_region_split(0, 740, 1280, 768, 1280));
    assert!(has_scroll_region_split(0, 768, 1265, 768, 1280));
}

#[test]
fn content_tiles_must_fit_fully_inside_scroll_region() {
    let ts = 64;
    let region_top = 84;
    let region_bottom = 740;
    let region_right = 1265;

    assert!(!is_content_tile_in_scroll_region(
        tiles::TileCoord::new(0, 1),
        ts,
        region_top,
        region_bottom,
        region_right,
    ));
    assert!(is_content_tile_in_scroll_region(
        tiles::TileCoord::new(0, 2),
        ts,
        region_top,
        region_bottom,
        region_right,
    ));
    assert!(!is_content_tile_in_scroll_region(
        tiles::TileCoord::new(0, 11),
        ts,
        region_top,
        region_bottom,
        region_right,
    ));
    assert!(!is_content_tile_in_scroll_region(
        tiles::TileCoord::new(19, 2),
        ts,
        region_top,
        region_bottom,
        region_right,
    ));
}

// ── Detect tests ────────────────────────────────────────────────────

#[test]
fn content_scroll_search_limit_tracks_large_cdp_deltas() {
    assert_eq!(content_scroll_search_limit_px(None), 256);
    assert_eq!(content_scroll_search_limit_px(Some(64)), 256);
    assert_eq!(content_scroll_search_limit_px(Some(-320)), 320);
    assert_eq!(content_scroll_search_limit_px(Some(512)), 384);
}

#[test]
fn wheel_scroll_prefers_content_when_pixels_confirm_it() {
    let selected = select_wheel_trusted_scroll(128, 1, Some((64, 0.88, true, Some(0.80))));
    assert_eq!(selected, Some((64, 0.88, "content", true, Some(0.80))));
}

#[test]
fn wheel_scroll_falls_back_to_cdp_when_content_disagrees() {
    let selected = select_wheel_trusted_scroll(128, 1, Some((-64, 0.90, false, Some(0.86))));
    assert_eq!(selected, Some((128, 1.0, "cdp", true, None)));
}

#[test]
fn wheel_scroll_rejects_cdp_when_input_direction_mismatches() {
    let selected = select_wheel_trusted_scroll(128, -1, None);
    assert_eq!(selected, None);
}
