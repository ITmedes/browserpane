//! Environment-based configuration for the bpane-host session.
//!
//! Centralises all `env::var` reads and provides typed, validated config
//! structs so the rest of the crate never touches raw environment strings.

#[cfg(test)]
mod tests;

use std::time::Duration;

use tracing::{info, warn};

use crate::tiles::emitter::TileCodec;

// ── H.264 mode ──────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum H264Mode {
    /// Keep the encoder process running continuously.
    Always,
    /// Start encoder only while tile emitter reports a video region.
    VideoTiles,
    /// Keep H.264 encoder disabled.
    Off,
}

impl H264Mode {
    pub fn from_env() -> Self {
        let raw = std::env::var("BPANE_H264_MODE").unwrap_or_else(|_| "video_tiles".to_string());
        match raw.trim().to_ascii_lowercase().as_str() {
            "always" => Self::Always,
            "" => Self::VideoTiles,
            "video_tiles" | "video-tiles" | "tiles" | "on_demand" | "ondemand" => Self::VideoTiles,
            "off" | "disabled" | "false" | "0" => Self::Off,
            _ => {
                warn!(
                    value = %raw,
                    "invalid BPANE_H264_MODE, defaulting to `always`"
                );
                Self::Always
            }
        }
    }

    pub fn starts_enabled(self) -> bool {
        matches!(self, Self::Always)
    }
}

// ── Tile codec / size helpers ───────────────────────────────────────

pub fn tile_codec_from_env() -> TileCodec {
    match std::env::var("BPANE_TILE_CODEC") {
        Ok(raw) => {
            let codec = TileCodec::from_str_lossy(&raw);
            info!(value = %raw, ?codec, "BPANE_TILE_CODEC");
            codec
        }
        Err(_) => TileCodec::Qoi,
    }
}

pub fn tile_size_from_env() -> u16 {
    const DEFAULT: u16 = 64;
    const MIN: u16 = 32;
    const MAX: u16 = 256;
    match std::env::var("BPANE_TILE_SIZE") {
        Ok(raw) => match raw.trim().parse::<u16>() {
            Ok(parsed) => {
                let bounded = parsed.clamp(MIN, MAX);
                let aligned = bounded & !0x0f;
                if aligned < MIN {
                    MIN
                } else {
                    if parsed != aligned {
                        warn!(
                            value = %raw,
                            effective = aligned,
                            "BPANE_TILE_SIZE rounded down to 16-pixel alignment"
                        );
                    }
                    aligned
                }
            }
            Err(_) => {
                warn!(value = %raw, "invalid BPANE_TILE_SIZE, using default");
                DEFAULT
            }
        },
        Err(_) => DEFAULT,
    }
}

// ── Generic env parsing ─────────────────────────────────────────────

pub fn env_u16_clamped(name: &str, default: u16, min: u16, max: u16) -> u16 {
    match std::env::var(name) {
        Ok(raw) => match raw.trim().parse::<u16>() {
            Ok(parsed) => parsed.clamp(min, max),
            Err(_) => {
                warn!(value = %raw, var = name, "invalid u16 env var, using default");
                default.clamp(min, max)
            }
        },
        Err(_) => default.clamp(min, max),
    }
}

pub fn env_u32_clamped(name: &str, default: u32, min: u32, max: u32) -> u32 {
    match std::env::var(name) {
        Ok(raw) => match raw.trim().parse::<u32>() {
            Ok(parsed) => parsed.clamp(min, max),
            Err(_) => {
                warn!(value = %raw, var = name, "invalid u32 env var, using default");
                default.clamp(min, max)
            }
        },
        Err(_) => default.clamp(min, max),
    }
}

pub fn env_bool(name: &str, default: bool) -> bool {
    match std::env::var(name) {
        Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => true,
            "0" | "false" | "no" | "off" => false,
            _ => {
                warn!(value = %raw, var = name, "invalid bool env var, using default");
                default
            }
        },
        Err(_) => default,
    }
}

pub fn env_f32_clamped(name: &str, default: f32, min: f32, max: f32) -> f32 {
    match std::env::var(name) {
        Ok(raw) => match raw.trim().parse::<f32>() {
            Ok(parsed) if parsed.is_finite() => parsed.clamp(min, max),
            _ => {
                warn!(value = %raw, var = name, "invalid f32 env var, using default");
                default.clamp(min, max)
            }
        },
        Err(_) => default.clamp(min, max),
    }
}

// ── Preflight ───────────────────────────────────────────────────────

pub fn preflight_checks() {
    #[cfg(target_os = "linux")]
    {
        if std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_err()
        {
            warn!("ffmpeg not found — H.264 capture will not work");
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        info!("running on non-Linux platform - using test backends");
    }
}

// ── Aggregate session config ────────────────────────────────────────

/// Read-only configuration for the tile capture thread.
/// Constructed once from environment variables at session start.
#[derive(Debug, Clone)]
pub struct TileCaptureConfig {
    pub h264_mode: H264Mode,
    pub tile_size: u16,
    pub tile_codec: TileCodec,
    pub chromium_wheel_step_px: u16,
    pub scroll_copy_quantum_px: u16,
    pub base_frame_interval: Duration,
    pub scroll_active_frame_interval: Duration,
    pub scroll_active_capture_frames: u8,
    pub min_cdp_video_width_px: u32,
    pub min_cdp_video_height_px: u32,
    pub min_cdp_video_area_ratio: f32,
    pub cdp_video_tile_margin: u16,
    pub scroll_thin_mode_enabled: bool,
    pub video_classification_enabled: bool,
}

impl TileCaptureConfig {
    /// Build config by reading all relevant `BPANE_*` environment variables.
    pub fn from_env() -> Self {
        let h264_mode = H264Mode::from_env();
        let chromium_wheel_step_px = env_u16_clamped("BPANE_CHROMIUM_WHEEL_STEP_PX", 64, 0, 512);
        let scroll_copy_quantum_px = env_u16_clamped(
            "BPANE_SCROLL_COPY_QUANTUM_PX",
            chromium_wheel_step_px,
            0,
            512,
        );
        Self {
            h264_mode,
            tile_size: tile_size_from_env(),
            tile_codec: tile_codec_from_env(),
            chromium_wheel_step_px,
            scroll_copy_quantum_px,
            base_frame_interval: Duration::from_millis(100),
            scroll_active_frame_interval: Duration::from_millis(env_u32_clamped(
                "BPANE_SCROLL_ACTIVE_FRAME_INTERVAL_MS",
                33,
                16,
                100,
            ) as u64),
            scroll_active_capture_frames: env_u32_clamped(
                "BPANE_SCROLL_ACTIVE_CAPTURE_FRAMES",
                8,
                0,
                32,
            ) as u8,
            min_cdp_video_width_px: env_u32_clamped("BPANE_CDP_MIN_VIDEO_WIDTH", 320, 2, 4096) & !1,
            min_cdp_video_height_px: env_u32_clamped("BPANE_CDP_MIN_VIDEO_HEIGHT", 180, 2, 4096)
                & !1,
            min_cdp_video_area_ratio: env_f32_clamped(
                "BPANE_CDP_MIN_VIDEO_AREA_RATIO",
                0.08,
                0.01,
                0.95,
            ),
            cdp_video_tile_margin: env_u16_clamped("BPANE_CDP_VIDEO_TILE_MARGIN", 1, 0, 3),
            scroll_thin_mode_enabled: env_bool("BPANE_SCROLL_THIN_MODE", false),
            video_classification_enabled: !matches!(h264_mode, H264Mode::Off),
        }
    }
}
