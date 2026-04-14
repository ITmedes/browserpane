use super::*;
use crate::capture::ffmpeg::CaptureRegion;

#[test]
fn css_pixels_scale_to_framebuffer_pixels() {
    // 1× scale: 100 CSS px → 100 screen px
    assert_eq!(scale_css_px_to_screen_px(100, 1000), 100);
    // 2× scale: 100 CSS px → 200 screen px
    assert_eq!(scale_css_px_to_screen_px(100, 2000), 200);
    // negative values
    assert_eq!(scale_css_px_to_screen_px(-50, 2000), -100);
    // zero scale_milli treated as 1
    assert_eq!(scale_css_px_to_screen_px(100, 0), 0);
}

#[test]
fn editable_hint_minimum_accepts_tiny_regions() {
    assert!(region_meets_editable_minimum(2, 2));
    assert!(!region_meets_editable_minimum(1, 2));
    assert!(!region_meets_editable_minimum(2, 1));
}

#[test]
fn expand_tile_bounds_adds_margin_and_clamps() {
    assert_eq!(expand_tile_bounds((2, 2, 5, 5), 1, 10, 10), (1, 1, 6, 6));
    // Clamps to grid edges
    assert_eq!(expand_tile_bounds((0, 0, 9, 9), 2, 10, 10), (0, 0, 9, 9));
}

#[test]
fn cdp_insert_text_payload_accepts_non_ascii_printable_chars() {
    // German umlaut ä (U+00E4) with no modifiers → Some
    assert!(cdp_insert_text_payload(0, 0x00E4).is_some());
    // Japanese hiragana あ (U+3042)
    assert!(cdp_insert_text_payload(0, 0x3042).is_some());
}

#[test]
fn cdp_insert_text_payload_rejects_ascii_and_shortcuts() {
    // ASCII 'a' → None (handled by keyboard)
    assert!(cdp_insert_text_payload(0, 0x61).is_none());
    // Control char → None
    assert!(cdp_insert_text_payload(0, 0x0A).is_none());
}

#[test]
fn extend_dirty_with_tile_bounds_adds_each_coord_once() {
    let mut dirty = vec![tiles::TileCoord::new(1, 1)];
    extend_dirty_with_tile_bounds(&mut dirty, (0, 0, 1, 1));
    // Should have (0,0), (1,0), (0,1), (1,1) — but (1,1) was already there
    assert_eq!(dirty.len(), 4);
}

#[test]
fn clamp_region_to_screen_ensures_even_dimensions() {
    let region = CaptureRegion {
        x: 0,
        y: 0,
        w: 101,
        h: 101,
    };
    let result = clamp_region_to_screen(region, 200, 200).unwrap();
    assert_eq!(result.w % 2, 0);
    assert_eq!(result.h % 2, 0);
}

#[test]
fn clamp_region_to_screen_rejects_tiny_screen() {
    let region = CaptureRegion {
        x: 0,
        y: 0,
        w: 10,
        h: 10,
    };
    assert!(clamp_region_to_screen(region, 1, 1).is_none());
}

#[test]
fn point_in_capture_region_works() {
    let region = CaptureRegion {
        x: 10,
        y: 20,
        w: 100,
        h: 50,
    };
    assert!(point_in_capture_region(50, 40, region));
    assert!(!point_in_capture_region(5, 40, region));
    assert!(!point_in_capture_region(50, 80, region));
}

#[test]
fn capture_region_tile_bounds_maps_correctly() {
    let region = CaptureRegion {
        x: 64,
        y: 128,
        w: 192,
        h: 64,
    };
    let (min_c, min_r, max_c, max_r) = capture_region_tile_bounds(region, 64, 20, 20);
    assert_eq!(min_c, 1);
    assert_eq!(min_r, 2);
    assert_eq!(max_c, 3);
    assert_eq!(max_r, 2);
}

#[test]
fn hash_tile_region_deterministic() {
    let frame = vec![42u8; 64 * 64 * 4];
    let stride = 64 * 4;
    let h1 = hash_tile_region(&frame, stride, 0, 0, 32, 32);
    let h2 = hash_tile_region(&frame, stride, 0, 0, 32, 32);
    assert_eq!(h1, h2);
}

#[test]
fn hash_tile_region_differs_for_different_content() {
    let mut frame = vec![0u8; 64 * 64 * 4];
    let stride = 64 * 4;
    let h1 = hash_tile_region(&frame, stride, 0, 0, 32, 32);
    frame[0] = 255;
    let h2 = hash_tile_region(&frame, stride, 0, 0, 32, 32);
    assert_ne!(h1, h2);
}

#[test]
fn region_meets_video_minimum_checks_all_constraints() {
    // Too narrow
    assert!(!region_meets_video_minimum(100, 200, 1920, 1080, 320, 180, 0.08));
    // Too short
    assert!(!region_meets_video_minimum(400, 100, 1920, 1080, 320, 180, 0.08));
    // Area too small
    assert!(!region_meets_video_minimum(320, 180, 1920, 1080, 320, 180, 0.50));
    // Meets all criteria
    assert!(region_meets_video_minimum(960, 540, 1920, 1080, 320, 180, 0.08));
}
