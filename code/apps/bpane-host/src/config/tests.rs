use super::*;

#[test]
fn h264_mode_defaults_to_video_tiles() {
    // With no env var set, from_env() should return VideoTiles.
    std::env::remove_var("BPANE_H264_MODE");
    assert_eq!(H264Mode::from_env(), H264Mode::VideoTiles);
}

#[test]
fn h264_mode_always_starts_enabled() {
    assert!(H264Mode::Always.starts_enabled());
}

#[test]
fn h264_mode_video_tiles_does_not_start_enabled() {
    assert!(!H264Mode::VideoTiles.starts_enabled());
}

#[test]
fn h264_mode_off_does_not_start_enabled() {
    assert!(!H264Mode::Off.starts_enabled());
}

#[test]
fn tile_size_default_is_64() {
    std::env::remove_var("BPANE_TILE_SIZE");
    assert_eq!(tile_size_from_env(), 64);
}

#[test]
fn tile_size_clamps_below_minimum() {
    std::env::set_var("BPANE_TILE_SIZE", "8");
    let size = tile_size_from_env();
    assert!(size >= 32, "tile size {size} should be >= 32");
    std::env::remove_var("BPANE_TILE_SIZE");
}

#[test]
fn tile_size_aligns_to_16() {
    std::env::set_var("BPANE_TILE_SIZE", "100");
    let size = tile_size_from_env();
    assert_eq!(size % 16, 0, "tile size {size} should be 16-aligned");
    assert_eq!(size, 96);
    std::env::remove_var("BPANE_TILE_SIZE");
}

#[test]
fn env_bool_parses_true_variants() {
    for val in &["1", "true", "yes", "on", "TRUE", "Yes", "ON"] {
        std::env::set_var("_TEST_BOOL_TRUE", val);
        assert!(env_bool("_TEST_BOOL_TRUE", false), "failed for {val}");
    }
    std::env::remove_var("_TEST_BOOL_TRUE");
}

#[test]
fn env_bool_parses_false_variants() {
    for val in &["0", "false", "no", "off", "FALSE", "No", "OFF"] {
        std::env::set_var("_TEST_BOOL_FALSE", val);
        assert!(!env_bool("_TEST_BOOL_FALSE", true), "failed for {val}");
    }
    std::env::remove_var("_TEST_BOOL_FALSE");
}

#[test]
fn env_bool_returns_default_when_unset() {
    std::env::remove_var("_TEST_BOOL_UNSET");
    assert!(env_bool("_TEST_BOOL_UNSET", true));
    assert!(!env_bool("_TEST_BOOL_UNSET", false));
}

#[test]
fn env_u16_clamped_respects_bounds() {
    std::env::set_var("_TEST_U16", "999");
    assert_eq!(env_u16_clamped("_TEST_U16", 50, 10, 100), 100);
    std::env::set_var("_TEST_U16", "5");
    assert_eq!(env_u16_clamped("_TEST_U16", 50, 10, 100), 10);
    std::env::set_var("_TEST_U16", "42");
    assert_eq!(env_u16_clamped("_TEST_U16", 50, 10, 100), 42);
    std::env::remove_var("_TEST_U16");
}

#[test]
fn env_f32_clamped_rejects_nan() {
    std::env::set_var("_TEST_F32", "NaN");
    assert_eq!(env_f32_clamped("_TEST_F32", 0.5, 0.0, 1.0), 0.5);
    std::env::remove_var("_TEST_F32");
}

#[test]
fn tile_capture_config_has_sane_defaults() {
    // Clear all env vars that could affect config.
    for var in &[
        "BPANE_H264_MODE",
        "BPANE_TILE_SIZE",
        "BPANE_TILE_CODEC",
        "BPANE_CHROMIUM_WHEEL_STEP_PX",
        "BPANE_SCROLL_COPY_QUANTUM_PX",
        "BPANE_SCROLL_ACTIVE_FRAME_INTERVAL_MS",
        "BPANE_SCROLL_ACTIVE_CAPTURE_FRAMES",
        "BPANE_CDP_MIN_VIDEO_WIDTH",
        "BPANE_CDP_MIN_VIDEO_HEIGHT",
        "BPANE_CDP_MIN_VIDEO_AREA_RATIO",
        "BPANE_VIDEO_CLICK_ARM_MS",
        "BPANE_SCROLL_THIN_MODE",
    ] {
        std::env::remove_var(var);
    }

    let cfg = TileCaptureConfig::from_env();
    assert_eq!(cfg.h264_mode, H264Mode::VideoTiles);
    assert_eq!(cfg.tile_size, 64);
    assert_eq!(cfg.base_frame_interval, Duration::from_millis(100));
    assert!(cfg.video_classification_enabled);
    assert!(!cfg.scroll_thin_mode_enabled);
    assert!(cfg.min_cdp_video_width_px % 2 == 0, "width must be even");
    assert!(cfg.min_cdp_video_height_px % 2 == 0, "height must be even");
}

#[test]
fn tile_capture_config_off_disables_classification() {
    std::env::set_var("BPANE_H264_MODE", "off");
    let cfg = TileCaptureConfig::from_env();
    assert_eq!(cfg.h264_mode, H264Mode::Off);
    assert!(!cfg.video_classification_enabled);
    std::env::remove_var("BPANE_H264_MODE");
}
