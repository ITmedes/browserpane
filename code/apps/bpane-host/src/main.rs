mod audio;
mod camera;
mod capture;
mod cdp_video;
mod clipboard;
mod config;
mod cursor;
mod display;
mod encode;
mod filetransfer;
mod input;
mod ipc;
mod region;
mod resize;
pub mod tiles;
mod video_classify;

use std::collections::HashSet;
use std::time::Duration;

use clap::Parser;
use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};
use tracing_subscriber::EnvFilter;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::{Frame, Message};
use bpane_protocol::{
    AudioFrame, ClipboardMessage, ControlMessage, SessionFlags, TileMessage, VideoDatagram,
    VideoTileInfo,
};

use input::{InputBackend, TestInputBackend};

const CONTENT_SCROLL_SEARCH_MAX_PX: usize = 384;
const SCROLL_RESIDUAL_FULL_REPAINT_RATIO_DEFAULT: f32 = 0.70;
const SCROLL_DEFER_REPAIR_MAX_INTERIOR_RATIO: f32 = 0.82;
const SCROLL_DEFER_REPAIR_MIN_SAVED_RATIO: f32 = 0.20;
const SCROLL_DEFER_REPAIR_MAX_ROW_SHIFT: i32 = 2;
// MIN_EDITABLE_HINT_*_PX moved to region module.

#[derive(Parser, Debug)]
#[command(name = "bpane-host", about = "BrowserPane host agent daemon")]
struct Args {
    /// Unix socket path for gateway IPC.
    #[arg(long, default_value = "/tmp/bpane.sock")]
    socket: String,

    /// Initial width.
    #[arg(long, default_value_t = 1280)]
    width: u32,

    /// Initial height.
    #[arg(long, default_value_t = 720)]
    height: u32,

    /// Target framerate (1-240).
    #[arg(long, default_value_t = 30)]
    fps: u32,
}

use config::{
    env_bool, env_f32_clamped, env_u16_clamped, env_u32_clamped, preflight_checks,
    tile_codec_from_env, tile_size_from_env, H264Mode,
};
use region::{
    capture_region_tile_bounds, cdp_insert_text_payload, clamp_region_to_screen,
    expand_tile_bounds, extend_dirty_with_tile_bounds, hash_tile_region,
    point_in_capture_region, region_meets_editable_minimum, region_meets_video_minimum,
    scale_css_px_to_screen_px,
};
use video_classify::{
    bbox_center_shift, bbox_iou, compute_tile_motion_features, is_photo_like_tile,
    TileMotionFeatures,
};

// Keep H.264 datagram payloads comfortably below the effective QUIC path MTU.
// Larger payloads can get dropped before they ever reach JS, which wipes out
// the entire fragmented NAL.
const SAFE_VIDEO_DATAGRAM_PAYLOAD: usize = 1100;

fn should_gate_video_delta_on_damage(tile_info: Option<VideoTileInfo>) -> bool {
    tile_info.is_none()
}

fn video_datagram_max_fragment_size(tile_info: Option<VideoTileInfo>) -> usize {
    let header_overhead = if tile_info.is_some() { 21 + 13 } else { 22 };
    SAFE_VIDEO_DATAGRAM_PAYLOAD
        .saturating_sub(header_overhead)
        .max(1)
}

fn unix_time_ms_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

// cdp_insert_text_payload moved to region module.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();

    if args.fps == 0 || args.fps > 240 {
        anyhow::bail!("--fps must be between 1 and 240, got {}", args.fps);
    }

    preflight_checks();

    let display_mode = display::detect_display_mode();
    info!("display mode: {:?}", display_mode);

    let audio_state = audio::detect_audio();
    let has_audio = matches!(audio_state, audio::AudioState::Available);
    if has_audio {
        info!("audio: available");
    } else if let audio::AudioState::Unavailable(ref reason) = audio_state {
        warn!("audio: unavailable ({reason})");
    }

    let camera_state = camera::detect_camera();
    let has_camera = matches!(camera_state, camera::CameraState::Available(_));
    match &camera_state {
        camera::CameraState::Available(device) => info!("camera: available ({device})"),
        camera::CameraState::Unavailable(reason) => warn!("camera: unavailable ({reason})"),
    }

    let mut flags = SessionFlags::new(
        SessionFlags::CLIPBOARD | SessionFlags::FILE_TRANSFER | SessionFlags::KEYBOARD_LAYOUT,
    );
    if has_audio {
        flags = SessionFlags::new(flags.0 | SessionFlags::AUDIO | SessionFlags::MICROPHONE);
    }
    if has_camera {
        flags = SessionFlags::new(flags.0 | SessionFlags::CAMERA);
    }

    let ipc_server = ipc::IpcServer::bind(&args.socket)?;

    loop {
        info!("waiting for gateway connection...");
        let (from_gateway, to_gateway) = ipc_server.accept().await?;

        if let Err(e) = run_session(&args, &display_mode, flags, from_gateway, to_gateway).await {
            warn!("session ended with error: {e}");
        }
        info!("session ended, ready for next connection");
    }
}

async fn run_session(
    args: &Args,
    display_mode: &display::DisplayMode,
    flags: SessionFlags,
    from_gateway: mpsc::Receiver<Frame>,
    to_gateway: mpsc::Sender<Frame>,
) -> anyhow::Result<()> {
    // Send SessionReady
    let ready = ControlMessage::SessionReady { version: 2, flags };
    to_gateway.send(ready.to_frame()).await?;

    let display_str = match display_mode {
        display::DisplayMode::X11 { display } | display::DisplayMode::Xvfb { display } => {
            display.clone()
        }
        _ => String::new(),
    };

    let use_display = !display_str.is_empty() && cfg!(target_os = "linux");
    let has_audio = flags.has(SessionFlags::AUDIO);

    // Keep production focused on one visual path: FFmpeg + tile overlay.
    // The direct-capture path remains in-source for experiments but is not used.
    if use_display {
        info!("using FFmpeg x11grab pipeline");
        run_ffmpeg_session(args, &display_str, has_audio, from_gateway, to_gateway).await
    } else {
        info!("no display available, using test backends");
        run_test_session(args, from_gateway, to_gateway).await
    }
}

/// Production session using FFmpeg for X11 capture + H.264 encode.
async fn run_ffmpeg_session(
    args: &Args,
    display: &str,
    has_audio: bool,
    mut from_gateway: mpsc::Receiver<Frame>,
    to_gateway: mpsc::Sender<Frame>,
) -> anyhow::Result<()> {
    // Hide the X cursor so only the streamed cursor is visible.
    #[allow(unused)]
    let _cursor_hider = match cursor::CursorHider::new(display) {
        Ok(h) => Some(h),
        Err(e) => {
            warn!("could not hide cursor: {e}");
            None
        }
    };

    // Shared cursor position for damage filtering (Phase 3)
    let cursor_pos = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));

    // Start cursor capture/forward task (with shared position)
    let cursor_task = cursor::spawn_cursor_task_with_pos(
        display.to_string(),
        to_gateway.clone(),
        Some(cursor_pos.clone()),
    );

    // Start audio capture task if available
    let audio_task = if has_audio {
        Some(audio::spawn_audio_capture(to_gateway.clone()))
    } else {
        None
    };

    // Start clipboard monitor task
    let clipboard_task = clipboard::spawn_clipboard_task(display.to_string(), to_gateway.clone());

    // Microphone input — initialized lazily on first AudioIn frame
    let mut mic_input: Option<audio::input::MicInput> = None;
    let mut camera_input: Option<camera::CameraInput> = None;

    let h264_mode = H264Mode::from_env();
    info!(?h264_mode, "h264 mode");
    let chromium_wheel_step_px = env_u16_clamped("BPANE_CHROMIUM_WHEEL_STEP_PX", 64, 0, 512);
    let scroll_copy_quantum_px = env_u16_clamped(
        "BPANE_SCROLL_COPY_QUANTUM_PX",
        chromium_wheel_step_px,
        0,
        512,
    );
    debug!(
        chromium_wheel_step_px,
        scroll_copy_quantum_px, "scroll quantization"
    );
    let base_frame_interval = std::time::Duration::from_millis(100);
    let scroll_active_frame_interval = std::time::Duration::from_millis(env_u32_clamped(
        "BPANE_SCROLL_ACTIVE_FRAME_INTERVAL_MS",
        33,
        16,
        100,
    ) as u64);
    let scroll_active_capture_frames =
        env_u32_clamped("BPANE_SCROLL_ACTIVE_CAPTURE_FRAMES", 8, 0, 32) as u8;
    debug!(
        base_frame_interval_ms = base_frame_interval.as_millis() as u64,
        scroll_active_frame_interval_ms = scroll_active_frame_interval.as_millis() as u64,
        scroll_active_capture_frames,
        "capture cadence"
    );

    // Spawn the FFmpeg pipeline in a dedicated thread.
    // It communicates via channels — no mutex contention.
    let (cmd_tx, nal_rx) = capture::ffmpeg::spawn_pipeline(
        display.to_string(),
        args.width,
        args.height,
        args.fps,
        h264_mode.starts_enabled(),
    )?;

    let mut input_backend: Box<dyn InputBackend> = if cfg!(target_os = "linux") {
        match input::mouse::MouseInjector::new()
            .and_then(|m| Ok((m, input::keyboard::KeyboardInjector::new()?)))
        {
            Ok((mouse, keyboard)) => {
                info!("input backend: XTEST (mouse + keyboard)");
                Box::new(input::CombinedInjector { mouse, keyboard }) as Box<dyn InputBackend>
            }
            Err(e) => {
                warn!("input backend fallback (test): {e}");
                Box::new(TestInputBackend::default())
            }
        }
    } else {
        info!("input backend: test (non-linux)");
        Box::new(TestInputBackend::default())
    };
    let mut nal_id: u32 = 0;

    // Bridge NAL units from the FFmpeg pipeline to the async gateway channel.
    // Keyframes are always forwarded (decoder needs them).
    // P-frames are gated by XDamage — only forwarded when the screen changed.
    // When tiles are actively producing frames, suppress H.264 to avoid flicker
    // (both systems render to the same canvas).
    let tiles_active = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let video_tile_info = std::sync::Arc::new(std::sync::Mutex::new(None::<VideoTileInfo>));
    let input_activity_ms = std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0));
    let browser_video_hint =
        std::sync::Arc::new(std::sync::Mutex::new(cdp_video::PageHintState::default()));
    let browser_video_hint_task =
        if matches!(h264_mode, H264Mode::VideoTiles) || chromium_wheel_step_px > 0 {
            Some(cdp_video::spawn_video_hint_task(browser_video_hint.clone()))
        } else {
            None
        };
    let tiles_active_for_bridge = tiles_active.clone();
    let video_tile_info_for_bridge = video_tile_info.clone();
    let display_for_bridge = display.to_string();
    let cursor_pos_for_bridge = cursor_pos.clone();
    let input_activity_for_bridge = input_activity_ms.clone();
    let cmd_tx_for_bridge = cmd_tx.clone();
    let (video_tx, mut video_rx) = mpsc::channel::<Frame>(128);
    let bridge = tokio::task::spawn_blocking(move || {
        let mut damage = capture::x11::DamageTracker::with_options(
            &display_for_bridge,
            Some(cursor_pos_for_bridge),
            None,
            Some(input_activity_for_bridge),
        )
        .ok()
        .flatten();

        if damage.is_some() {
            debug!("damage gating: active");
        }

        let mut bridge_encoder_disabled = false;

        while let Ok(encoded) = nal_rx.recv() {
            // When tiles are handling the screen, disable FFmpeg to avoid
            // wasting capture+encode CPU on frames that would be discarded.
            let tiles_on = tiles_active_for_bridge.load(std::sync::atomic::Ordering::Relaxed);
            if tiles_on && !bridge_encoder_disabled {
                bridge_encoder_disabled = true;
                let _ = cmd_tx_for_bridge.send(capture::ffmpeg::PipelineCmd::SetEnabled(false));
                trace!("bridge: disabled encoder (tiles active)");
            } else if !tiles_on && bridge_encoder_disabled {
                bridge_encoder_disabled = false;
                let _ = cmd_tx_for_bridge.send(capture::ffmpeg::PipelineCmd::SetEnabled(true));
                trace!("bridge: re-enabled encoder (tiles inactive)");
            }
            if tiles_on {
                if let Some(dt) = damage.as_mut() {
                    dt.reset();
                }
                continue;
            }

            let tile_info = {
                let guard = match video_tile_info_for_bridge.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                *guard
            };

            // Keyframes always pass through.
            // ROI video tiles are already explicitly armed by the tile pipeline;
            // gating their deltas on XDamage can collapse the stream to
            // keyframes-only if XDamage misses video surface updates.
            if !encoded.is_keyframe && should_gate_video_delta_on_damage(tile_info) {
                let has_damage = match damage.as_mut() {
                    Some(dt) => dt.poll(),
                    None => true,
                };
                if !has_damage {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    continue;
                }
            }

            nal_id = nal_id.wrapping_add(1);
            let fragments = VideoDatagram::fragment_with_tile(
                nal_id,
                encoded.is_keyframe,
                encoded.pts_us,
                &encoded.data,
                video_datagram_max_fragment_size(tile_info),
                tile_info,
            );
            for frag in &fragments {
                let frame = Frame::new(ChannelId::Video, frag.encode());
                if video_tx.blocking_send(frame).is_err() {
                    return;
                }
            }

            if let Some(dt) = damage.as_mut() {
                dt.reset();
            }
        }
        debug!("bridge: NAL receive channel closed");
    });

    // Forward video frames to the gateway
    let video_gateway = to_gateway.clone();
    let video_fwd = tokio::spawn(async move {
        while let Some(frame) = video_rx.recv().await {
            if video_gateway.send(frame).await.is_err() {
                break;
            }
        }
    });

    // ── Scroll event forwarding to tile thread ──────────────────────
    // The tile thread needs to know when scroll events were injected so it
    // can detect pixel displacement and shift the tile grid accordingly.
    let (scroll_tx, scroll_rx) = std::sync::mpsc::channel::<(i16, i16)>();
    // Left-clicks arm video encoding for a short window.
    let (video_click_tx, video_click_rx) =
        std::sync::mpsc::channel::<(u16, u16, std::time::Instant)>();
    // Key presses arm a short QOI boost around the focused editable region.
    let (text_input_tx, text_input_rx) = std::sync::mpsc::channel::<std::time::Instant>();
    // Browser-reported cache misses invalidate sender cache assumptions.
    let (cache_miss_tx, cache_miss_rx) = std::sync::mpsc::channel::<(u32, u16, u16, u64)>();

    // ── Tile capture thread ─────────────────────────────────────────
    // Runs alongside FFmpeg: captures screen via X11 GetImage, hashes
    // 64×64 tiles, emits Fill/QOI/CacheHit frames on the Tiles channel.
    // This gives lossless tile delivery for static content while FFmpeg
    // handles motion via H.264.
    let display_for_tiles = display.to_string();
    let (tile_tx, mut tile_rx) = mpsc::channel::<Frame>(256);
    let tile_w = args.width;
    let tile_h = args.height;
    let h264_mode_for_tile = h264_mode;
    let cmd_tx_for_tile = cmd_tx.clone();
    let tiles_active_for_tile = tiles_active.clone();
    let video_tile_info_for_tile = video_tile_info.clone();
    let browser_video_hint_for_tile = browser_video_hint.clone();
    let input_activity_for_tile = input_activity_ms.clone();
    let tile_size_for_tiles = tile_size_from_env();
    let tile_codec_for_tiles = tile_codec_from_env();
    let scroll_copy_quantum_px_for_tile = scroll_copy_quantum_px;
    let tile_thread = tokio::task::spawn_blocking(move || {
        use capture::CaptureBackend;

        let mut cap = match capture::x11::X11CaptureBackend::new(&display_for_tiles, tile_w, tile_h)
        {
            Ok(c) => c,
            Err(e) => {
                warn!("tile capture: X11 backend init failed: {e}");
                return;
            }
        };

        // Use actual screen resolution from X11, not args (screen may have resized).
        let (init_w, init_h) = cap.resolution();
        let mut screen_w = init_w as u16;
        let mut screen_h = init_h as u16;
        let tile_size: u16 = tile_size_for_tiles;
        let tile_codec = tile_codec_for_tiles;
        info!(
            "tile capture: active ({}x{}, tile_size={}, codec={:?})",
            screen_w, screen_h, tile_size, tile_codec
        );

        // ── Scroll displacement detection state ───────────────────
        let mut prev_frame: Option<Vec<u8>> = None;
        let mut content_origin_y: i64 = 0;
        let mut grid_offset_y: u16 = 0;

        let mut grid = tiles::TileGrid::new(screen_w, screen_h, tile_size);
        let mut emitter = tiles::emitter::TileEmitter::with_codec(grid.cols, grid.rows, tile_codec);
        let mut h264_enabled = h264_mode_for_tile.starts_enabled();
        let mut ffmpeg_capture_region: Option<capture::ffmpeg::CaptureRegion> = None;
        let mut pending_capture_region: Option<capture::ffmpeg::CaptureRegion> = None;
        let mut pending_capture_region_streak: u8 = 0;

        // Per-tile tracking for robust video classification.
        // Uses multi-signal scoring + hysteresis to avoid flip/flop.
        let total_tiles = grid.cols as usize * grid.rows as usize;
        let mut prev_hashes: Vec<u64> = vec![0; total_tiles];
        let mut video_scores: Vec<i8> = vec![0; total_tiles];
        let mut non_candidate_streaks: Vec<u8> = vec![0; total_tiles];
        let mut video_hold_frames: Vec<u8> = vec![0; total_tiles];
        let mut video_latched: Vec<bool> = vec![false; total_tiles];
        let mut candidate_mask: Vec<bool> = vec![false; total_tiles];
        let mut changed_mask: Vec<bool> = vec![false; total_tiles];
        let mut text_like_mask: Vec<bool> = vec![false; total_tiles];
        const VIDEO_ENTER_SCORE: i8 = 5;
        const VIDEO_EXIT_SCORE: i8 = -1;
        const VIDEO_MAX_SCORE: i8 = 20;
        const VIDEO_DECAY_STREAK: u8 = 18;
        const VIDEO_MIN_HOLD_FRAMES: u8 = 12;
        const MIN_SCROLL_DY_PX: i32 = 2;
        const INPUT_MIN_SCROLL_DY_PX: i32 = 4;
        const MAX_CDP_SCROLL_DY_PX: i64 = CONTENT_SCROLL_SEARCH_MAX_PX as i64;
        const CDP_CONTENT_DY_DIVERGENCE_LOG_PX: i32 = 3;
        const INPUT_SCROLL_MIN_CONFIDENCE: f32 = 0.80;
        const NO_INPUT_SCROLL_MIN_CONFIDENCE: f32 = 0.86;
        const SCROLL_SUPPRESS_VIDEO_FRAMES: u8 = 14;
        const SCROLL_RESIDUAL_FULL_REPAINT_RATIO: f32 = SCROLL_RESIDUAL_FULL_REPAINT_RATIO_DEFAULT;
        // Reduce per-tick bandwidth on sub-tile scroll when residual diff is
        // broad/noisy: send exposed strip while scrolling, then one repair.
        const SCROLL_THIN_MODE_RESIDUAL_RATIO: f32 = 0.14;
        const SCROLL_THIN_REPAIR_QUIET_FRAMES: u8 = 5;
        let scroll_thin_mode_enabled = env_bool("BPANE_SCROLL_THIN_MODE", false);
        const REGION_RECONFIG_STABLE_FRAMES: u8 = 2;
        const REGION_RECONFIG_MIN_INTERVAL_MS: u64 = 350;
        const REGION_MIN_CANDIDATES: u32 = 6;
        const REGION_DENSE_CANDIDATES: u32 = 18;
        const MIN_VIDEO_BBOX_WIDTH_TILES: u16 = 4;
        const MIN_VIDEO_BBOX_HEIGHT_TILES: u16 = 3;
        const MIN_VIDEO_BBOX_AREA_RATIO: f32 = 0.10;
        // Video hints stay strict so tiny previews do not activate ROI H.264.
        // Focused editable controls stay on the tile path and use QOI overrides.
        let min_cdp_video_width_px =
            env_u32_clamped("BPANE_CDP_MIN_VIDEO_WIDTH", 320, 2, 4096) & !1;
        let min_cdp_video_height_px =
            env_u32_clamped("BPANE_CDP_MIN_VIDEO_HEIGHT", 180, 2, 4096) & !1;
        let min_cdp_video_area_ratio =
            env_f32_clamped("BPANE_CDP_MIN_VIDEO_AREA_RATIO", 0.08, 0.01, 0.95);
        const EDITABLE_QOI_TILE_MARGIN: u16 = 2;
        const EDITABLE_HINT_HOLD_MS: u64 = 450;
        const KEY_INPUT_QOI_BOOST_MS: u64 = 800;
        const MIN_CHANGED_VIDEO_TILES_FOR_H264: u32 = 8;
        let video_click_arm_ms =
            env_u32_clamped("BPANE_VIDEO_CLICK_ARM_MS", 8_000, 250, 60_000) as u64;
        const CLICK_LATCH_RESET_FRAMES: u8 = 20;
        const H264_MIN_ON_DURATION_MS: u64 = 900;
        let video_classification_enabled = !matches!(h264_mode_for_tile, H264Mode::Off);

        // Bounding box stability: video regions have a stable spatial footprint
        // while scroll shifts the changing region each frame. Track the bbox of
        // video-like changing tiles and require temporal stability before
        // allowing VideoMotion classification.
        let mut prev_video_bbox: Option<(u16, u16, u16, u16)> = None; // (min_col, min_row, max_col, max_row)
        let mut stable_bbox_frames: u8 = 0;
        let mut scroll_cooldown_frames: u8 = 0;
        let mut last_left_click: Option<(u16, u16, std::time::Instant)> = None;
        let mut click_latched_video: bool = false;
        let mut cdp_hint_absent_streak: u8 = 0;
        let mut last_key_input_at: Option<std::time::Instant> = None;
        let mut last_editable_hint: Option<capture::ffmpeg::CaptureRegion> = None;
        let mut last_editable_hint_at: Option<std::time::Instant> = None;
        // Scroll residual telemetry (host-side cumulative counters).
        let mut scroll_residual_batches_total: u64 = 0;
        let mut scroll_residual_fallback_full_total: u64 = 0;
        let mut scroll_potential_tiles_total: u64 = 0;
        let mut scroll_residual_tiles_total: u64 = 0;
        let mut scroll_saved_tiles_total: u64 = 0;
        let mut scroll_thin_mode_active: bool = false;
        let mut scroll_residual_was_active: bool = false;
        let mut scroll_quiet_frames: u8 = 0;
        // Cached scroll region from last detected scroll frame — used to split
        // static/content tiles during reconciliation and non-scroll frames.
        let mut last_scroll_region_top: u16 = 0;
        let mut last_scroll_region_bottom: u16 = screen_h;
        let mut last_scroll_region_right: u16 = screen_w;
        let mut scroll_thin_batches_total: u64 = 0;
        let mut scroll_thin_repairs_total: u64 = 0;
        let mut scroll_active_capture_frames_remaining: u8 = 0;
        let mut last_cdp_hint_seq: u64 = 0;
        let mut last_cdp_scroll_y: Option<i64> = None;
        // Anchor: CDP scrollY value when content_origin_y was known-correct.
        // Used to recalibrate content_origin_y against CDP absolute position
        // and eliminate accumulated drift from content-based detection.
        let mut cdp_scroll_anchor: Option<(i64, i64)> = None; // (cdp_scroll_y, content_origin_y)

        // Send grid config so the client knows the tile layout.
        let grid_frame = emitter.emit_grid_config(&grid);
        if tile_tx.blocking_send(grid_frame).is_err() {
            return;
        }

        // Optional XDamage tracking — skip frames with no changes.
        let mut damage = capture::x11::DamageTracker::with_options(
            &display_for_tiles,
            None,
            None,
            Some(input_activity_for_tile),
        )
        .ok()
        .flatten();
        if damage.is_some() {
            debug!("tile capture: XDamage tracking active");
        }

        let mut last_capture = std::time::Instant::now();
        let mut last_resize_check = std::time::Instant::now() - std::time::Duration::from_secs(1);
        let mut last_region_reconfig_at =
            std::time::Instant::now() - std::time::Duration::from_secs(10);
        let mut last_h264_toggle_at = std::time::Instant::now();

        loop {
            let frame_interval = select_capture_frame_interval(
                base_frame_interval,
                scroll_active_frame_interval,
                scroll_active_capture_frames_remaining,
            );
            // Sleep until next frame interval is due (minimum 16ms for event coalescing).
            let now = std::time::Instant::now();
            let since_last = now.duration_since(last_capture);
            let sleep_dur = if since_last >= frame_interval {
                std::time::Duration::from_millis(16)
            } else {
                (frame_interval - since_last).max(std::time::Duration::from_millis(16))
            };
            std::thread::sleep(sleep_dur);

            let mut force_refresh = false;
            while let Ok((frame_seq, col, row, hash)) = cache_miss_rx.try_recv() {
                emitter.handle_cache_miss(col, row, hash);
                force_refresh = true;
                trace!(
                    frame_seq,
                    col,
                    row,
                    hash,
                    "tile cache miss reported by client"
                );
            }

            let has_damage = match damage.as_mut() {
                Some(dt) => dt.poll(),
                None => true,
            };
            if !has_damage && !force_refresh {
                continue;
            }

            let now = std::time::Instant::now();
            if now.duration_since(last_capture) < frame_interval {
                continue;
            }
            last_capture = now;

            // Check if screen resolution changed (xrandr resize by FFmpeg pipeline).
            // Only issue the X11 GetGeometry round-trip every ~500ms to avoid
            // per-frame latency on the hot path. Cached values are used otherwise.
            let (cur_w, cur_h) =
                if now.duration_since(last_resize_check) >= std::time::Duration::from_millis(500) {
                    last_resize_check = now;
                    cap.refresh_screen_size()
                } else {
                    cap.query_screen_size()
                };
            if cur_w as u16 != screen_w || cur_h as u16 != screen_h {
                screen_w = cur_w as u16;
                screen_h = cur_h as u16;
                grid = tiles::TileGrid::new(screen_w, screen_h, tile_size);
                emitter = tiles::emitter::TileEmitter::with_codec(grid.cols, grid.rows, tile_codec);
                let new_total = grid.cols as usize * grid.rows as usize;
                prev_hashes = vec![0; new_total];
                video_scores = vec![0; new_total];
                non_candidate_streaks = vec![0; new_total];
                video_hold_frames = vec![0; new_total];
                video_latched = vec![false; new_total];
                candidate_mask = vec![false; new_total];
                changed_mask = vec![false; new_total];
                text_like_mask = vec![false; new_total];
                prev_video_bbox = None;
                stable_bbox_frames = 0;
                scroll_cooldown_frames = 0;
                scroll_thin_mode_active = false;
                scroll_residual_was_active = false;
                scroll_quiet_frames = 0;
                last_cdp_hint_seq = 0;
                last_cdp_scroll_y = None;
                cdp_scroll_anchor = None;
                prev_frame = None;
                content_origin_y = 0;
                grid_offset_y = 0;
                pending_capture_region = None;
                pending_capture_region_streak = 0;
                last_region_reconfig_at = std::time::Instant::now();
                last_h264_toggle_at = std::time::Instant::now();
                if ffmpeg_capture_region.take().is_some() {
                    let _ = cmd_tx_for_tile.send(capture::ffmpeg::PipelineCmd::SetRegion(None));
                }
                {
                    let mut guard = match video_tile_info_for_tile.lock() {
                        Ok(g) => g,
                        Err(poisoned) => poisoned.into_inner(),
                    };
                    *guard = None;
                }
                debug!("tile capture: resized to {}x{}", screen_w, screen_h);

                // Send new grid config
                let grid_frame = emitter.emit_grid_config(&grid);
                if tile_tx.blocking_send(grid_frame).is_err() {
                    return;
                }
                continue;
            }

            let raw = match cap.capture_region_raw(0, 0, screen_w, screen_h) {
                Ok(data) => data,
                Err(e) => {
                    warn!("tile capture: GetImage failed: {e}");
                    continue;
                }
            };

            // Pixels stay in native BGRA format — hashing is format-agnostic,
            // and the per-tile BGRA→RGBA swap happens only for QOI-encoded tiles
            // in the emitter (a small subset of total pixels).
            let rgba = raw;

            grid.advance_frame();
            let stride = screen_w as usize * 4;

            // ── Scroll displacement detection ─────────────────────
            // Drain pending scroll events (used only for logging).
            let mut pending_scrolls = 0i32;
            let mut pending_scroll_dy_sum = 0i32;
            while let Ok((_, dy)) = scroll_rx.try_recv() {
                pending_scrolls += 1;
                pending_scroll_dy_sum += dy as i32;
            }
            while let Ok((x, y, ts)) = video_click_rx.try_recv() {
                last_left_click = Some((x, y, ts));
            }
            while let Ok(ts) = text_input_rx.try_recv() {
                last_key_input_at = Some(ts);
            }

            let prev_for_analysis = prev_frame.as_deref();
            let cdp_hint_snapshot = {
                let guard = match browser_video_hint_for_tile.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                *guard
            };
            let cdp_hint_region_kind = cdp_hint_snapshot.region_kind;
            let cdp_hint_region_raw = cdp_hint_snapshot.video_region.and_then(|region| {
                clamp_region_to_screen(region, screen_w as u32, screen_h as u32)
            });
            let cdp_video_region_hint_sized =
                if matches!(cdp_hint_region_kind, cdp_video::HintRegionKind::Video) {
                    cdp_hint_region_raw.filter(|region| {
                        region_meets_video_minimum(
                            region.w,
                            region.h,
                            screen_w as u32,
                            screen_h as u32,
                            min_cdp_video_width_px,
                            min_cdp_video_height_px,
                            min_cdp_video_area_ratio,
                        )
                    })
                } else {
                    None
                };
            let cdp_editable_region_hint =
                if matches!(cdp_hint_region_kind, cdp_video::HintRegionKind::Editable) {
                    cdp_hint_region_raw
                        .filter(|region| region_meets_editable_minimum(region.w, region.h))
                } else {
                    None
                };
            if let Some(region) = cdp_editable_region_hint {
                last_editable_hint = Some(region);
                last_editable_hint_at = Some(now);
            } else if last_editable_hint_at
                .map(|ts| {
                    now.duration_since(ts) > std::time::Duration::from_millis(EDITABLE_HINT_HOLD_MS)
                })
                .unwrap_or(false)
            {
                last_editable_hint = None;
                last_editable_hint_at = None;
            }
            let key_input_qoi_boost = last_key_input_at
                .map(|ts| {
                    now.duration_since(ts)
                        <= std::time::Duration::from_millis(KEY_INPUT_QOI_BOOST_MS)
                })
                .unwrap_or(false);
            let editable_qoi_region = cdp_editable_region_hint.or_else(|| {
                if key_input_qoi_boost
                    && last_editable_hint_at
                        .map(|ts| {
                            now.duration_since(ts)
                                <= std::time::Duration::from_millis(EDITABLE_HINT_HOLD_MS)
                        })
                        .unwrap_or(false)
                {
                    last_editable_hint
                } else {
                    None
                }
            });
            let editable_qoi_tile_bounds = editable_qoi_region.map(|region| {
                expand_tile_bounds(
                    capture_region_tile_bounds(region, tile_size, grid.cols, grid.rows),
                    EDITABLE_QOI_TILE_MARGIN,
                    grid.cols,
                    grid.rows,
                )
            });
            let mut cdp_scroll_dy_px: Option<i16> = None;
            let cdp_scale_milli = cdp_hint_snapshot.device_scale_factor_milli.max(1);
            if let Some(scroll_y_css) = cdp_hint_snapshot.scroll_y {
                let scroll_y = scale_css_px_to_screen_px(scroll_y_css, cdp_scale_milli);
                if let Some(prev_scroll_y) = last_cdp_scroll_y {
                    let dy = scroll_y.saturating_sub(prev_scroll_y);
                    if dy != 0 {
                        let clamped = dy.clamp(-MAX_CDP_SCROLL_DY_PX, MAX_CDP_SCROLL_DY_PX) as i16;
                        if clamped != 0 {
                            cdp_scroll_dy_px = Some(clamped);
                        }
                    }
                }
                last_cdp_scroll_y = Some(scroll_y);
            } else {
                last_cdp_scroll_y = None;
            }
            if cdp_scroll_dy_px.is_none()
                && cdp_hint_snapshot.update_seq != 0
                && cdp_hint_snapshot.update_seq != last_cdp_hint_seq
                && cdp_hint_snapshot.scroll_delta_y != 0
            {
                let clamped = scale_css_px_to_screen_px(
                    cdp_hint_snapshot.scroll_delta_y as i64,
                    cdp_scale_milli,
                )
                .clamp(-MAX_CDP_SCROLL_DY_PX, MAX_CDP_SCROLL_DY_PX)
                    as i16;
                if clamped != 0 {
                    cdp_scroll_dy_px = Some(clamped);
                }
            }
            if cdp_hint_snapshot.update_seq != 0 {
                last_cdp_hint_seq = cdp_hint_snapshot.update_seq;
            }

            // Content-based scroll detection: compare current frame to
            // previous using vertical column matching. Runs on every frame
            // (not gated on input events) so it catches all scroll sources:
            // mouse wheel, scrollbar drag, keyboard, programmatic scrolls.
            // Industry standard (RDP/SPICE/VNC) intercepts OS drawing
            // commands; since we lack compositor hooks, framebuffer
            // comparison is the correct alternative.
            let mut strong_scroll_observed = false;
            let mut detected_scroll_dy_px: Option<i16> = None;
            let input_scroll_dir = (-pending_scroll_dy_sum).signum();
            let cdp_scroll_dir = cdp_scroll_dy_px.map(|dy| (dy as i32).signum()).unwrap_or(0);
            let hint_scroll_dir = if input_scroll_dir != 0 {
                input_scroll_dir
            } else {
                cdp_scroll_dir
            };
            let min_scroll_dy_px = if pending_scrolls > 0 {
                INPUT_MIN_SCROLL_DY_PX
            } else {
                MIN_SCROLL_DY_PX
            };
            let mut content_scroll: Option<(i16, f32, bool, Option<f32>)> = None;
            let content_scroll_search_px = content_scroll_search_limit_px(cdp_scroll_dy_px);
            if let Some(prev) = prev_for_analysis {
                if let Some((detected_dy, confidence)) = detect_column_scroll(
                    &rgba,
                    prev,
                    stride,
                    screen_w as usize,
                    screen_h as usize,
                    content_scroll_search_px,
                ) {
                    let detected_scroll_dir = detected_dy.signum();
                    let direction_matches = hint_scroll_dir == 0
                        || detected_scroll_dir == 0
                        || hint_scroll_dir == detected_scroll_dir;
                    // Scroll hints are soft: matching direction lowers confidence threshold.
                    let min_confidence = if hint_scroll_dir != 0 && direction_matches {
                        INPUT_SCROLL_MIN_CONFIDENCE
                    } else {
                        NO_INPUT_SCROLL_MIN_CONFIDENCE
                    };
                    let trusted =
                        detected_dy.abs() >= min_scroll_dy_px && confidence >= min_confidence;
                    if trusted {
                        content_scroll = Some((
                            detected_dy as i16,
                            confidence,
                            direction_matches,
                            Some(min_confidence),
                        ));
                    } else {
                        trace!(
                            source = "content",
                            dy = detected_dy,
                            confidence = format!("{:.2}", confidence),
                            scrolls = pending_scrolls,
                            input_dy_sum = pending_scroll_dy_sum,
                            input_dir = input_scroll_dir,
                            cdp_dir = cdp_scroll_dir,
                            dir_match = direction_matches,
                            min_scroll_dy_px,
                            min_confidence = format!("{:.2}", min_confidence),
                            "ignored tiny scroll displacement"
                        );
                    }
                }
            }
            let mut trusted_scroll: Option<(i16, f32, &'static str, bool, Option<f32>)> = None;
            let mut detected_scroll_frame: Option<(
                i16,
                f32,
                &'static str,
                bool,
                Option<f32>,
                i32,
                u16,
                u16,
                u16,
            )> = None;

            if let Some(detected_dy) = cdp_scroll_dy_px {
                let cdp_abs = (detected_dy as i32).abs();
                if cdp_abs >= min_scroll_dy_px {
                    if pending_scrolls > 0 {
                        if let Some((
                            selected_dy,
                            selected_confidence,
                            source,
                            direction_matches,
                            min_confidence,
                        )) = select_wheel_trusted_scroll(
                            detected_dy,
                            input_scroll_dir,
                            content_scroll,
                        ) {
                            if source == "content" {
                                let dy_gap = ((selected_dy as i32) - (detected_dy as i32)).abs();
                                if dy_gap >= CDP_CONTENT_DY_DIVERGENCE_LOG_PX {
                                    trace!(
                                        cdp_dy = detected_dy,
                                        content_dy = selected_dy,
                                        dy_gap,
                                        confidence = format!("{:.2}", selected_confidence),
                                        "wheel scroll dy diverged; preferring content (pixel-aligned)"
                                    );
                                }
                            }
                            trusted_scroll = Some((
                                selected_dy,
                                selected_confidence,
                                source,
                                direction_matches,
                                min_confidence,
                            ));
                        } else {
                            let detected_scroll_dir = (detected_dy as i32).signum();
                            let direction_matches = input_scroll_dir == 0
                                || detected_scroll_dir == 0
                                || input_scroll_dir == detected_scroll_dir;
                            trace!(
                                source = "cdp",
                                dy = detected_dy,
                                scrolls = pending_scrolls,
                                input_dy_sum = pending_scroll_dy_sum,
                                input_dir = input_scroll_dir,
                                dir_match = direction_matches,
                                min_scroll_dy_px,
                                "ignored cdp scroll hint with mismatched direction"
                            );
                        }
                    } else if let Some((
                        content_dy,
                        content_confidence,
                        content_direction_matches,
                        content_min_confidence,
                    )) = content_scroll
                    {
                        let cdp_dir = (detected_dy as i32).signum();
                        let content_dir = (content_dy as i32).signum();
                        if cdp_dir == 0 || content_dir == 0 || cdp_dir == content_dir {
                            let dy_gap = ((content_dy as i32) - (detected_dy as i32)).abs();
                            if dy_gap >= CDP_CONTENT_DY_DIVERGENCE_LOG_PX {
                                trace!(
                                    cdp_dy = detected_dy,
                                    content_dy,
                                    dy_gap,
                                    confidence = format!("{:.2}", content_confidence),
                                    "cdp/content scroll dy diverged; preferring content (pixel-aligned)"
                                );
                            }
                            // Prefer content-based dy: it is derived from the
                            // actual captured frames and therefore pixel-aligned
                            // with the residual comparison.  CDP dy comes from a
                            // different timing domain and may not match the
                            // framebuffer state at capture time.
                            trusted_scroll = Some((
                                content_dy,
                                content_confidence,
                                "content",
                                content_direction_matches,
                                content_min_confidence,
                            ));
                        } else {
                            trace!(
                                cdp_dy = detected_dy,
                                content_dy,
                                confidence = format!("{:.2}", content_confidence),
                                "cdp/content direction mismatch; preferring content"
                            );
                            trusted_scroll = Some((
                                content_dy,
                                content_confidence,
                                "content",
                                content_direction_matches,
                                content_min_confidence,
                            ));
                        }
                    } else {
                        // CDP reports a scroll but content-based detection cannot
                        // confirm it in the captured pixels.  This typically means
                        // the frame was captured mid-render — the browser's scrollY
                        // has changed but the framebuffer doesn't yet reflect the
                        // full shift.  Entering scroll mode here would use a delta
                        // that doesn't match the actual pixels, causing the residual
                        // comparison to fail and triggering a costly full repaint.
                        //
                        // Instead, skip scroll mode and emit only the XDamage dirty
                        // tiles as a normal frame.  The next frame capture will
                        // likely see the completed render and content detection will
                        // confirm the scroll then.
                        trace!(
                            source = "cdp-passive-skipped",
                            dy = detected_dy,
                            scrolls = pending_scrolls,
                            cdp_dir = cdp_scroll_dir,
                            "cdp scroll unconfirmed by content detection; skipping scroll mode"
                        );
                    }
                } else if pending_scrolls > 0 {
                    trace!(
                        source = "cdp",
                        dy = detected_dy,
                        scrolls = pending_scrolls,
                        input_dy_sum = pending_scroll_dy_sum,
                        input_dir = input_scroll_dir,
                        min_scroll_dy_px,
                        "ignored tiny cdp scroll hint"
                    );
                }
            }

            if trusted_scroll.is_none() {
                if let Some((dy, confidence, direction_matches, min_confidence)) = content_scroll {
                    trusted_scroll =
                        Some((dy, confidence, "content", direction_matches, min_confidence));
                }
            }

            if let Some((detected_dy, confidence, source, direction_matches, min_confidence)) =
                trusted_scroll
            {
                content_origin_y += detected_dy as i64;
                // Re-anchor after each trusted scroll so drift correction stays fresh.
                if let Some(cdp_scroll_y_css) = cdp_hint_snapshot.scroll_y {
                    let cdp_scroll_y = scale_css_px_to_screen_px(cdp_scroll_y_css, cdp_scale_milli);
                    cdp_scroll_anchor = Some((cdp_scroll_y, content_origin_y));
                }
                grid_offset_y = 0;

                // Keep tiles in fixed screen-space. ScrollCopy is only used
                // for whole-tile moves, so row-shift is derived directly from
                // the observed scroll delta rather than a moving tile grid.
                let row_shift = if detected_dy as i32 % tile_size as i32 == 0 {
                    detected_dy as i32 / tile_size as i32
                } else {
                    0
                };

                // Determine scroll region from CDP viewport hint.
                // If available, limit canvas shift to viewport content area
                // so browser toolbar and scrollbar don't jump.
                let (scroll_region_top, scroll_region_bottom, scroll_region_right) = {
                    if let Some(vp) = cdp_hint_snapshot.viewport {
                        (
                            (vp.y as u16).min(screen_h),
                            ((vp.y + vp.h) as u16).min(screen_h),
                            ((vp.x + vp.w) as u16).min(screen_w),
                        )
                    } else {
                        (0, screen_h, screen_w)
                    }
                };

                detected_scroll_frame = Some((
                    detected_dy,
                    confidence,
                    source,
                    direction_matches,
                    min_confidence,
                    row_shift,
                    scroll_region_top,
                    scroll_region_bottom,
                    scroll_region_right,
                ));
                strong_scroll_observed = true;
                detected_scroll_dy_px = Some(detected_dy);
            } else {
                // Recalibrate content_origin_y against CDP absolute scrollY
                // to eliminate accumulated drift from content-based detection.
                // This ensures scroll-back to a previous position produces
                // identical grid_offset_y → identical tile hashes → cache hits.
                //
                // IMPORTANT: only apply when no scroll is detected this frame.
                // If both CDP correction and scroll detection fire in the same
                // frame, the scroll delta is double-counted — CDP correction
                // adds the delta, then scroll detection adds it again — causing
                // grid_offset_y to be 2x off and producing ghost/double text.
                if let Some(cdp_scroll_y_css) = cdp_hint_snapshot.scroll_y {
                    let cdp_scroll_y = scale_css_px_to_screen_px(cdp_scroll_y_css, cdp_scale_milli);
                    if let Some((anchor_scroll_y, anchor_origin)) = cdp_scroll_anchor {
                        let expected_origin = anchor_origin + (cdp_scroll_y - anchor_scroll_y);
                        let drift = content_origin_y - expected_origin;
                        if drift != 0 {
                            tracing::trace!(
                                drift,
                                content_origin_y,
                                expected_origin,
                                cdp_scroll_y,
                                "correcting content_origin_y drift from CDP"
                            );
                            content_origin_y = expected_origin;
                            grid_offset_y = 0;
                        }
                    } else {
                        // Establish anchor on first CDP reading
                        cdp_scroll_anchor = Some((cdp_scroll_y, content_origin_y));
                    }
                }
            }
            let cdp_scroll_observed = cdp_scroll_dy_px.is_some();
            scroll_active_capture_frames_remaining = next_scroll_active_capture_frames(
                scroll_active_capture_frames_remaining,
                scroll_active_capture_frames,
                pending_scrolls,
                strong_scroll_observed,
                cdp_scroll_observed,
            );
            if pending_scrolls > 0 || strong_scroll_observed || cdp_scroll_observed {
                // Scroll-like motion always wins over video classification.
                // Keep video suppressed for a short quiet period.
                scroll_cooldown_frames = SCROLL_SUPPRESS_VIDEO_FRAMES;
                stable_bbox_frames = 0;
                prev_video_bbox = None;
                scroll_quiet_frames = 0;
            } else if scroll_cooldown_frames > 0 {
                scroll_cooldown_frames -= 1;
                scroll_quiet_frames = scroll_quiet_frames.saturating_add(1);
            } else {
                scroll_quiet_frames = scroll_quiet_frames.saturating_add(1);
            }
            let cdp_video_region_hint_candidate = if scroll_cooldown_frames == 0 {
                cdp_video_region_hint_sized
            } else {
                None
            };
            let click_matches_region = cdp_video_region_hint_candidate
                .zip(last_left_click)
                .map(|(region, (x, y, ts))| {
                    now.duration_since(ts) <= std::time::Duration::from_millis(video_click_arm_ms)
                        && point_in_capture_region(x, y, region)
                })
                .unwrap_or(false);
            if click_matches_region {
                click_latched_video = true;
            }
            if cdp_video_region_hint_candidate.is_some() {
                cdp_hint_absent_streak = 0;
            } else {
                cdp_hint_absent_streak = cdp_hint_absent_streak.saturating_add(1);
                if cdp_hint_absent_streak >= CLICK_LATCH_RESET_FRAMES {
                    click_latched_video = false;
                }
            }
            let cdp_click_armed = click_latched_video;
            let cdp_video_region_hint = if cdp_click_armed {
                cdp_video_region_hint_candidate
            } else {
                None
            };
            let cdp_hint_tile_bounds = cdp_video_region_hint
                .map(|region| capture_region_tile_bounds(region, tile_size, grid.cols, grid.rows));

            // ── Per-tile change detection (two-pass) ─────────────
            // Pass 1: hash each tile, extract motion features for changed
            // tiles, identify video candidates, compute candidate bbox.
            let cols = grid.cols as usize;
            candidate_mask.fill(false);
            changed_mask.fill(false);
            text_like_mask.fill(false);
            let mut bbox_min_col: u16 = u16::MAX;
            let mut bbox_max_col: u16 = 0;
            let mut bbox_min_row: u16 = u16::MAX;
            let mut bbox_max_row: u16 = 0;
            let mut candidate_count = 0u32;
            let mut strong_candidate_count = 0u32;

            for row in 0..grid.rows {
                for col in 0..grid.cols {
                    let idx = row as usize * cols + col as usize;
                    let tx = col as usize * tile_size as usize;
                    let ty = row as usize * tile_size as usize;
                    let tw = (tile_size as usize).min(screen_w as usize - tx);
                    let th = (tile_size as usize).min(screen_h as usize - ty);
                    let hash = hash_tile_region(&rgba, stride, tx, ty, tw, th);
                    let changed = prev_hashes[idx] != 0 && hash != prev_hashes[idx];
                    changed_mask[idx] = changed;

                    if changed
                        && video_classification_enabled
                        && cdp_video_region_hint.is_some()
                        && scroll_cooldown_frames == 0
                    {
                        let features = compute_tile_motion_features(
                            &rgba,
                            prev_for_analysis,
                            stride,
                            tx,
                            ty,
                            tw,
                            th,
                        );
                        // Text/UI is typically edge-heavy and lower entropy.
                        // Video/canvas is usually high motion + richer entropy.
                        text_like_mask[idx] =
                            features.edge_density > 0.22 && features.entropy_hint < 0.45;
                        let photo_like = is_photo_like_tile(&rgba, stride, tx, ty, tw, th, 16);
                        let video_like = features.change_ratio > 0.23
                            && features.motion_magnitude > 0.045
                            && features.entropy_hint > 0.16
                            && features.edge_density < 0.74
                            && photo_like;
                        let strong_video_like = features.change_ratio > 0.30
                            && features.motion_magnitude > 0.07
                            && features.entropy_hint > 0.20
                            && features.edge_density < 0.70
                            && photo_like;

                        if video_like {
                            candidate_mask[idx] = true;
                            candidate_count += 1;
                            if strong_video_like {
                                strong_candidate_count += 1;
                            }
                            bbox_min_col = bbox_min_col.min(col);
                            bbox_max_col = bbox_max_col.max(col);
                            bbox_min_row = bbox_min_row.min(row);
                            bbox_max_row = bbox_max_row.max(row);
                        }
                    }
                    prev_hashes[idx] = hash;
                }
            }

            // Bounding box stability check:
            // Video regions stay roughly stable in position/shape, while
            // scrolling tends to translate the bbox frame to frame.
            let current_bbox = if candidate_count > 0 {
                Some((bbox_min_col, bbox_min_row, bbox_max_col, bbox_max_row))
            } else {
                None
            };

            let bbox_stable = match (current_bbox, prev_video_bbox) {
                (Some(cur), Some(prev)) => {
                    let iou = bbox_iou(cur, prev);
                    let center_shift = bbox_center_shift(cur, prev);
                    iou > 0.55 || center_shift <= 1.25
                }
                _ => false,
            };
            if bbox_stable {
                stable_bbox_frames = stable_bbox_frames.saturating_add(1);
            } else {
                stable_bbox_frames = 0;
            }
            prev_video_bbox = current_bbox;
            let bbox_tile_area = current_bbox
                .map(|(min_c, min_r, max_c, max_r)| {
                    ((max_c - min_c + 1) as u32).saturating_mul((max_r - min_r + 1) as u32)
                })
                .unwrap_or(0);
            let bbox_density = if bbox_tile_area > 0 {
                candidate_count as f32 / bbox_tile_area as f32
            } else {
                0.0
            };
            let (bbox_w_tiles, bbox_h_tiles) = current_bbox
                .map(|(min_c, min_r, max_c, max_r)| (max_c - min_c + 1, max_r - min_r + 1))
                .unwrap_or((0, 0));
            let total_tile_area = (grid.cols as u32).saturating_mul(grid.rows as u32).max(1);
            let bbox_area_ratio = bbox_tile_area as f32 / total_tile_area as f32;
            let large_stable_region = bbox_w_tiles >= MIN_VIDEO_BBOX_WIDTH_TILES
                && bbox_h_tiles >= MIN_VIDEO_BBOX_HEIGHT_TILES
                && bbox_area_ratio >= MIN_VIDEO_BBOX_AREA_RATIO;
            let region_stable = video_classification_enabled
                && cdp_video_region_hint.is_some()
                && scroll_cooldown_frames == 0
                && candidate_count >= REGION_MIN_CANDIDATES
                && large_stable_region
                && ((stable_bbox_frames >= 4 && bbox_density > 0.34)
                    || candidate_count >= REGION_DENSE_CANDIDATES
                    || strong_candidate_count >= 8);

            // Pass 2: update per-tile motion scores + hysteresis and classify.
            let mut latched_video_tiles: Vec<tiles::TileCoord> = Vec::new();
            let mut cdp_motion_tiles: u32 = 0;
            for row in 0..grid.rows {
                for col in 0..grid.cols {
                    let idx = row as usize * cols + col as usize;
                    let in_cdp_video_hint = cdp_hint_tile_bounds
                        .map(|(min_col, min_row, max_col, max_row)| {
                            col >= min_col && col <= max_col && row >= min_row && row <= max_row
                        })
                        .unwrap_or(false);
                    let cdp_motion_candidate =
                        in_cdp_video_hint && changed_mask[idx] && !text_like_mask[idx];
                    if cdp_motion_candidate {
                        cdp_motion_tiles = cdp_motion_tiles.saturating_add(1);
                    }
                    if region_stable && candidate_mask[idx] {
                        video_scores[idx] = (video_scores[idx] + 4).min(VIDEO_MAX_SCORE);
                        non_candidate_streaks[idx] = 0;
                        if video_latched[idx] {
                            video_hold_frames[idx] = VIDEO_MIN_HOLD_FRAMES;
                        }
                    } else if cdp_motion_candidate {
                        video_scores[idx] = (video_scores[idx] + 2).min(VIDEO_MAX_SCORE);
                        non_candidate_streaks[idx] = 0;
                        if video_latched[idx] {
                            video_hold_frames[idx] = VIDEO_MIN_HOLD_FRAMES;
                        }
                    } else if changed_mask[idx] {
                        video_scores[idx] = (video_scores[idx] - 1).max(-VIDEO_MAX_SCORE);
                        non_candidate_streaks[idx] = non_candidate_streaks[idx].saturating_add(1);
                    } else {
                        video_scores[idx] = (video_scores[idx] - 1).max(-VIDEO_MAX_SCORE);
                        non_candidate_streaks[idx] = non_candidate_streaks[idx].saturating_add(1);
                    }

                    if !(region_stable && candidate_mask[idx] || cdp_motion_candidate)
                        && video_hold_frames[idx] > 0
                    {
                        video_hold_frames[idx] -= 1;
                    }

                    if !video_classification_enabled
                        || cdp_video_region_hint.is_none()
                        || scroll_cooldown_frames > 0
                    {
                        video_latched[idx] = false;
                        video_hold_frames[idx] = 0;
                        video_scores[idx] = video_scores[idx].min(0);
                    } else if video_latched[idx] {
                        if video_hold_frames[idx] == 0
                            && (video_scores[idx] <= VIDEO_EXIT_SCORE
                                || non_candidate_streaks[idx] >= VIDEO_DECAY_STREAK)
                        {
                            video_latched[idx] = false;
                            video_hold_frames[idx] = 0;
                        }
                    } else if (region_stable && video_scores[idx] >= VIDEO_ENTER_SCORE)
                        || (cdp_motion_candidate && video_scores[idx] >= (VIDEO_ENTER_SCORE - 1))
                    {
                        video_latched[idx] = true;
                        video_hold_frames[idx] = VIDEO_MIN_HOLD_FRAMES;
                    }

                    if let Some(tile) = grid.get_mut(tiles::TileCoord::new(col, row)) {
                        tile.dirty = true;
                        tile.classification = if video_latched[idx] {
                            tiles::TileClass::VideoMotion
                        } else if changed_mask[idx] && text_like_mask[idx] {
                            tiles::TileClass::TextScroll
                        } else {
                            tiles::TileClass::Static
                        };
                    }
                    if video_latched[idx] {
                        latched_video_tiles.push(tiles::TileCoord::new(col, row));
                    }
                }
            }
            // Emit tiles for all positions, including extra bottom row when
            // grid offset is active (partial tile at bottom edge).
            let emit_rows = if grid_offset_y > 0 {
                grid.rows + 1
            } else {
                grid.rows
            };
            let full_emit_coords: Vec<tiles::TileCoord> = (0..emit_rows)
                .flat_map(|r| (0..grid.cols).map(move |c| tiles::TileCoord::new(c, r)))
                .collect();
            // Narrow dirty set to tiles overlapping XDamage bounding box.
            // Tiles outside the damage region are guaranteed unchanged by the
            // X server, so we skip their extraction + hashing entirely.
            // During scroll or force-refresh we fall back to all tiles.
            let mut all_dirty: Vec<tiles::TileCoord> = if !force_refresh {
                if let Some(ref dt) = damage {
                    if let Some((dx, dy, dw, dh)) = dt.damage_bounding_box() {
                        let ts = tile_size as u16;
                        let dx2 = dx.saturating_add(dw);
                        let dy2 = dy.saturating_add(dh);
                        full_emit_coords
                            .iter()
                            .copied()
                            .filter(|coord| {
                                let tx = coord.col * ts;
                                let ty = coord.row * ts;
                                let tx2 = tx.saturating_add(ts);
                                let ty2 = ty.saturating_add(ts);
                                // AABB overlap test
                                tx < dx2 && tx2 > dx && ty < dy2 && ty2 > dy
                            })
                            .collect()
                    } else {
                        full_emit_coords.clone()
                    }
                } else {
                    full_emit_coords.clone()
                }
            } else {
                full_emit_coords.clone()
            };
            let mut scroll_residual_ratio: Option<f32> = None;
            let mut scroll_residual_fallback_full = false;
            let mut scroll_residual_tiles_frame: Option<usize> = None;
            let mut scroll_potential_tiles_frame: Option<usize> = None;
            let mut scroll_saved_tiles_frame: Option<usize> = None;
            let mut scroll_saved_ratio_frame: Option<f32> = None;
            let mut scroll_emit_ratio_frame: Option<f32> = None;
            let mut scroll_thin_mode_frame = false;
            let mut scroll_thin_repair_frame = false;
            if let (Some(scroll_dy), Some(prev)) = (detected_scroll_dy_px, prev_for_analysis) {
                let scroll_row_shift = detected_scroll_frame
                    .map(|(_, _, _, _, _, row_shift, _, _, _)| row_shift)
                    .unwrap_or(0);
                // Get scroll region bounds for partitioning tiles into
                // content (scrollable) and chrome (static).  Use freshly
                // detected values when available, fall back to cached.
                let srt_for_split = detected_scroll_frame
                    .map(|(_, _, _, _, _, _, srt, _, _)| srt)
                    .unwrap_or(last_scroll_region_top);
                let srb_for_split = detected_scroll_frame
                    .map(|(_, _, _, _, _, _, _, srb, _)| srb)
                    .unwrap_or(screen_h);
                let srr_for_split = detected_scroll_frame
                    .map(|(_, _, _, _, _, _, _, _, srr)| srr)
                    .unwrap_or(last_scroll_region_right);
                let ts = tile_size as u16;

                // Partition: content tiles are below chrome header and left
                // of scrollbar; everything else is chrome / static.
                // Chrome exists regardless of sub-tile offset; partition
                // whenever we have a detected scroll region.
                let have_split = has_scroll_region_split(
                    srt_for_split,
                    srb_for_split,
                    srr_for_split,
                    screen_h,
                    screen_w,
                );
                let (content_emit_coords, chrome_emit_coords): (
                    Vec<tiles::TileCoord>,
                    Vec<tiles::TileCoord>,
                ) = if have_split {
                    full_emit_coords.iter().partition(|coord| {
                        // Only tiles fully inside the scrollable viewport are
                        // eligible for ScrollCopy reuse. Boundary tiles that
                        // overlap the header, bottom seam, or scrollbar remain
                        // raw/static so the host never assumes partially moved
                        // tiles are already correct on the client.
                        is_content_tile_in_scroll_region(
                            **coord,
                            ts,
                            srt_for_split,
                            srb_for_split,
                            srr_for_split,
                        )
                    })
                } else {
                    (full_emit_coords.clone(), Vec::new())
                };

                let residual_coords = content_emit_coords.clone();
                let residual = build_scroll_residual_emit_coords(
                    &rgba,
                    prev,
                    stride,
                    &grid,
                    grid_offset_y,
                    scroll_dy,
                    &residual_coords,
                );

                let potential_tiles = residual_coords.len();
                let residual_tiles = residual.len();
                let residual_ratio = if potential_tiles == 0 {
                    1.0
                } else {
                    residual_tiles as f32 / potential_tiles as f32
                };
                scroll_residual_batches_total = scroll_residual_batches_total.saturating_add(1);
                scroll_potential_tiles_total =
                    scroll_potential_tiles_total.saturating_add(potential_tiles as u64);
                scroll_residual_tiles_total =
                    scroll_residual_tiles_total.saturating_add(residual_tiles as u64);
                scroll_residual_tiles_frame = Some(residual_tiles);
                scroll_potential_tiles_frame = Some(potential_tiles);
                scroll_residual_ratio = Some(residual_ratio);

                // Fallback decision: only triggers if the residual ratio
                // for INTERIOR content tiles (excluding newly exposed edges)
                // exceeds the threshold, indicating scroll detection inaccuracy.
                let exposed_tiles = build_scroll_exposed_strip_emit_coords(
                    &grid,
                    grid_offset_y,
                    scroll_dy,
                    &residual_coords,
                );
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
                let quantized_scroll_copy =
                    can_emit_scroll_copy(scroll_dy, scroll_copy_quantum_px_for_tile, tile_size);
                let saved_tiles = potential_tiles.saturating_sub(residual_tiles);
                let defer_scroll_repair = should_defer_scroll_repair(
                    quantized_scroll_copy,
                    interior_ratio,
                    saved_tiles,
                    potential_tiles,
                    scroll_row_shift,
                );
                if !quantized_scroll_copy {
                    trace!(
                        dy = scroll_dy,
                        scroll_copy_quantum_px = scroll_copy_quantum_px_for_tile,
                        "scroll copy suppressed for non-quantized delta"
                    );
                    scroll_residual_fallback_full = true;
                    scroll_residual_fallback_full_total =
                        scroll_residual_fallback_full_total.saturating_add(1);
                    scroll_saved_tiles_frame = Some(0);
                    scroll_saved_ratio_frame = Some(0.0);
                    scroll_emit_ratio_frame = Some(1.0);
                    scroll_thin_mode_active = false;
                    scroll_residual_was_active = false;
                    // Restore full dirty set — XDamage narrowing may have
                    // excluded tiles that need a full repaint after scroll.
                    all_dirty = full_emit_coords.clone();
                } else if interior_ratio > SCROLL_RESIDUAL_FULL_REPAINT_RATIO
                    && !defer_scroll_repair
                {
                    trace!(
                        dy = scroll_dy,
                        row_shift = scroll_row_shift,
                        interior_ratio = format!("{:.2}", interior_ratio),
                        saved_tiles,
                        potential_tiles,
                        "scroll copy suppressed by residual full repaint threshold"
                    );
                    scroll_residual_fallback_full = true;
                    scroll_residual_fallback_full_total =
                        scroll_residual_fallback_full_total.saturating_add(1);
                    scroll_saved_tiles_frame = Some(0);
                    scroll_saved_ratio_frame = Some(0.0);
                    scroll_emit_ratio_frame = Some(1.0);
                    scroll_thin_mode_active = false;
                    scroll_residual_was_active = false;
                    // Restore full dirty set — XDamage narrowing may have
                    // excluded tiles that need a full repaint after scroll.
                    all_dirty = full_emit_coords.clone();
                } else {
                    // Dirty set = residual content + chrome.
                    all_dirty = residual;
                    all_dirty.extend(chrome_emit_coords.iter().copied());

                    // Force content tiles overlapping the client-side exposed
                    // strip into the dirty set.  The residual analysis compares
                    // pixels (white == white on a uniform page) so these tiles
                    // pass the check, but ScrollCopy cleared their canvas region.
                    // Without this, they'd be skipped → black band.
                    {
                        let (exp_start, exp_end) = if scroll_dy > 0 {
                            let start =
                                (srb_for_split as i32 - scroll_dy as i32).max(srt_for_split as i32);
                            (start, srb_for_split as i32)
                        } else {
                            let end = (srt_for_split as i32 + (-scroll_dy) as i32)
                                .min(srb_for_split as i32);
                            (srt_for_split as i32, end)
                        };
                        for &coord in &content_emit_coords {
                            if all_dirty.contains(&coord) {
                                continue;
                            }
                            let rect = offset_tile_rect_for_emit(coord, &grid, grid_offset_y);
                            if rect.w == 0 || rect.h == 0 {
                                continue;
                            }
                            let tile_top = rect.y as i32;
                            let tile_bot = tile_top + rect.h as i32;
                            if tile_bot > exp_start && tile_top < exp_end {
                                all_dirty.push(coord);
                            }
                        }
                    }
                    scroll_saved_tiles_total =
                        scroll_saved_tiles_total.saturating_add(saved_tiles as u64);
                    scroll_saved_tiles_frame = Some(saved_tiles);
                    if potential_tiles > 0 {
                        scroll_saved_ratio_frame =
                            Some(saved_tiles as f32 / potential_tiles as f32);
                        scroll_emit_ratio_frame =
                            Some(residual_tiles as f32 / potential_tiles as f32);
                    } else {
                        scroll_saved_ratio_frame = Some(0.0);
                        scroll_emit_ratio_frame = Some(1.0);
                    }
                    scroll_residual_was_active = saved_tiles > 0;
                    if defer_scroll_repair {
                        trace!(
                            dy = scroll_dy,
                            row_shift = scroll_row_shift,
                            interior_ratio = format!("{:.2}", interior_ratio),
                            saved_tiles,
                            potential_tiles,
                            "scroll copy accepted with deferred repair"
                        );
                    }

                    // If sub-tile residual explodes, prioritize exposed strip
                    // during active scrolling and defer one full repair frame.
                    let sub_tile_scroll = (scroll_dy.unsigned_abs() as u16) < tile_size;
                    let residual_tiles_min_for_thin = (grid.cols as usize * 2).max(12);
                    let residual_large_for_sub_tile = residual_ratio
                        >= SCROLL_THIN_MODE_RESIDUAL_RATIO
                        || residual_tiles >= residual_tiles_min_for_thin;
                    if sub_tile_scroll && residual_large_for_sub_tile {
                        let strip_dirty = build_scroll_exposed_strip_emit_coords(
                            &grid,
                            grid_offset_y,
                            scroll_dy,
                            &residual_coords,
                        );
                        if scroll_thin_mode_enabled && !strip_dirty.is_empty() {
                            // Keep chrome tiles in dirty set for static emit.
                            all_dirty = strip_dirty;
                            all_dirty.extend(chrome_emit_coords.iter().copied());
                            scroll_thin_mode_active = true;
                            scroll_thin_mode_frame = true;
                            scroll_thin_batches_total = scroll_thin_batches_total.saturating_add(1);
                        } else {
                            scroll_thin_mode_active = false;
                        }
                    } else {
                        scroll_thin_mode_active = false;
                    }
                }
            } else if (scroll_thin_mode_active || scroll_residual_was_active)
                && scroll_quiet_frames >= SCROLL_THIN_REPAIR_QUIET_FRAMES
            {
                // Reconcile: after scroll quiets, force a full tile emit to
                // correct any accumulated errors from ScrollCopy + residual skipping.
                // This ensures tiles skipped by residual analysis (whose last_hashes
                // are stale) get properly re-evaluated and updated.
                all_dirty = full_emit_coords.clone();
                scroll_thin_mode_active = false;
                scroll_residual_was_active = false;
                scroll_thin_repair_frame = true;
                scroll_thin_repairs_total = scroll_thin_repairs_total.saturating_add(1);
            }

            let emit_scroll_copy =
                should_emit_scroll_copy(scroll_residual_fallback_full, scroll_saved_tiles_frame);
            if let Some((
                detected_dy,
                confidence,
                source,
                direction_matches,
                min_confidence,
                row_shift,
                scroll_region_top,
                scroll_region_bottom,
                scroll_region_right,
            )) = detected_scroll_frame
            {
                if emit_scroll_copy {
                    // Only shift hashes when ScrollCopy is actually sent —
                    // keeps last_hashes consistent with the client canvas.
                    if row_shift != 0 {
                        emitter.shift_hashes(row_shift, grid.rows);
                    }
                    // Always zero exposed strip when sending ScrollCopy,
                    // even for sub-tile scrolls (row_shift == 0).  The client
                    // keeps that strip stale until repair tiles arrive; if we
                    // don't zero the corresponding hashes, L1 skip can keep
                    // the stale strip visible indefinitely.
                    emitter.zero_exposed_strip(
                        detected_dy,
                        scroll_region_top,
                        scroll_region_bottom,
                        tile_size,
                        grid_offset_y,
                    );
                    let scroll_frame = bpane_protocol::TileMessage::ScrollCopy {
                        dx: 0,
                        dy: detected_dy,
                        region_top: scroll_region_top,
                        region_bottom: scroll_region_bottom,
                        region_right: scroll_region_right,
                    }
                    .to_frame();
                    if tile_tx.blocking_send(scroll_frame).is_err() {
                        return;
                    }
                }

                let offset_frame = bpane_protocol::TileMessage::GridOffset {
                    offset_x: 0,
                    offset_y: grid_offset_y as i16,
                }
                .to_frame();
                if tile_tx.blocking_send(offset_frame).is_err() {
                    return;
                }

                tracing::debug!(
                    source,
                    dy = detected_dy,
                    confidence = format!("{:.2}", confidence),
                    offset_y = grid_offset_y,
                    row_shift = row_shift,
                    scroll_copy = emit_scroll_copy,
                    scrolls = pending_scrolls,
                    input_dy_sum = pending_scroll_dy_sum,
                    input_dir = input_scroll_dir,
                    cdp_dir = cdp_scroll_dir,
                    dir_match = direction_matches,
                    min_scroll_dy_px,
                    min_confidence = min_confidence.map(|c| format!("{:.2}", c)),
                    "scroll detected"
                );
            }

            // Export cumulative host-side residual telemetry to client for
            // direct Scroll Health reporting in test dashboards.
            let to_u32_sat = |v: u64| -> u32 { v.min(u32::MAX as u64) as u32 };
            let scroll_stats_frame = bpane_protocol::TileMessage::ScrollStats {
                scroll_batches_total: to_u32_sat(scroll_residual_batches_total),
                scroll_full_fallbacks_total: to_u32_sat(scroll_residual_fallback_full_total),
                scroll_potential_tiles_total: to_u32_sat(scroll_potential_tiles_total),
                scroll_saved_tiles_total: to_u32_sat(scroll_saved_tiles_total),
            }
            .to_frame();
            if tile_tx.blocking_send(scroll_stats_frame).is_err() {
                return;
            }

            // Split dirty tiles into static (browser chrome) and content (scrolling
            // viewport). Static tiles are emitted at raw framebuffer positions
            // with a separate hash table that is never shifted, so browser
            // header/scrollbar tiles achieve L1 cache hits across scroll frames.
            if let Some((_, _, _, _, _, _, srt, srb, srr)) = detected_scroll_frame {
                last_scroll_region_top = srt;
                last_scroll_region_bottom = srb;
                last_scroll_region_right = srr;
            }
            if key_input_qoi_boost {
                if let Some(bounds) = editable_qoi_tile_bounds {
                    extend_dirty_with_tile_bounds(&mut all_dirty, bounds);
                }
            }
            let use_static_split = has_scroll_region_split(
                last_scroll_region_top,
                last_scroll_region_bottom,
                last_scroll_region_right,
                screen_h,
                screen_w,
            );
            let (content_dirty, static_dirty): (Vec<tiles::TileCoord>, Vec<tiles::TileCoord>) =
                if use_static_split {
                    let ts = tile_size as u16;
                    let srt = last_scroll_region_top;
                    let srb = last_scroll_region_bottom;
                    let srr = last_scroll_region_right;
                    all_dirty.iter().partition(|coord| {
                        // Must stay in sync with the residual-analysis
                        // partitioning above.
                        is_content_tile_in_scroll_region(**coord, ts, srt, srb, srr)
                    })
                } else {
                    (all_dirty, Vec::new())
                };

            let result = emitter.emit_frame(
                &rgba,
                stride,
                &content_dirty,
                &grid,
                grid_offset_y,
                editable_qoi_tile_bounds,
            );

            // Emit static (chrome) tiles separately with offset_y=0.
            let static_frames = if !static_dirty.is_empty() {
                let draw_mode_off = bpane_protocol::TileMessage::TileDrawMode {
                    apply_offset: false,
                }
                .to_frame();
                let ts = tile_size as u16;
                let boundary_col = if use_static_split
                    && last_scroll_region_right < screen_w
                    && last_scroll_region_right % ts != 0
                {
                    Some(last_scroll_region_right / ts)
                } else {
                    None
                };
                let boundary_top_row = if use_static_split
                    && last_scroll_region_top > 0
                    && last_scroll_region_top % ts != 0
                {
                    Some(last_scroll_region_top / ts)
                } else {
                    None
                };
                let boundary_bottom_row = if use_static_split
                    && last_scroll_region_bottom < screen_h
                    && last_scroll_region_bottom % ts != 0
                {
                    Some(last_scroll_region_bottom / ts)
                } else {
                    None
                };
                let tiles = emitter.emit_static_tiles(
                    &rgba,
                    stride,
                    &static_dirty,
                    &grid,
                    boundary_col,
                    boundary_top_row,
                    boundary_bottom_row,
                    editable_qoi_tile_bounds,
                );
                let draw_mode_on =
                    bpane_protocol::TileMessage::TileDrawMode { apply_offset: true }.to_frame();
                let mut out = Vec::with_capacity(tiles.len() + 2);
                out.push(draw_mode_off);
                out.extend(tiles);
                out.push(draw_mode_on);
                out
            } else {
                Vec::new()
            };
            grid.clear_dirty();

            let static_tile_count = static_frames.len().saturating_sub(2); // minus DrawMode on/off
            let static_bytes: usize = static_frames.iter().map(|f| f.payload.len()).sum();

            let s = &result.stats;
            if s.fills > 0 || s.qoi_tiles > 0 || s.cache_hits > 0 || scroll_residual_ratio.is_some()
            {
                let scroll_saved_rate_total = if scroll_potential_tiles_total > 0 {
                    Some(scroll_saved_tiles_total as f32 / scroll_potential_tiles_total as f32)
                } else {
                    None
                };
                tracing::trace!(
                    skipped = s.skipped,
                    fills = s.fills,
                    cache_hits = s.cache_hits,
                    qoi = s.qoi_tiles,
                    video_changed = s.video_tiles,
                    video_latched = latched_video_tiles.len(),
                    cdp_motion_tiles,
                    cdp_hint_raw = cdp_hint_region_raw.is_some(),
                    cdp_video_hint = cdp_video_region_hint.is_some(),
                    editable_qoi = editable_qoi_tile_bounds.is_some(),
                    key_input_qoi_boost,
                    click_armed = cdp_click_armed,
                    scroll_suppress = scroll_cooldown_frames,
                    scroll_residual_ratio = scroll_residual_ratio.map(|r| format!("{:.2}", r)),
                    scroll_residual_full = scroll_residual_fallback_full,
                    scroll_residual_tiles = scroll_residual_tiles_frame,
                    scroll_potential_tiles = scroll_potential_tiles_frame,
                    scroll_saved_tiles = scroll_saved_tiles_frame,
                    scroll_saved_ratio = scroll_saved_ratio_frame.map(|r| format!("{:.2}", r)),
                    scroll_emit_ratio = scroll_emit_ratio_frame.map(|r| format!("{:.2}", r)),
                    scroll_batches_total = scroll_residual_batches_total,
                    scroll_full_fallbacks_total = scroll_residual_fallback_full_total,
                    scroll_potential_tiles_total,
                    scroll_residual_tiles_total,
                    scroll_saved_tiles_total,
                    scroll_saved_rate_total = scroll_saved_rate_total.map(|r| format!("{:.2}", r)),
                    scroll_thin_mode = scroll_thin_mode_frame,
                    scroll_thin_repair = scroll_thin_repair_frame,
                    scroll_thin_active = scroll_thin_mode_active,
                    scroll_thin_batches_total,
                    scroll_thin_repairs_total,
                    qoi_kb = s.qoi_bytes / 1024,
                    static_tiles = static_tile_count,
                    static_bytes,
                    content_dirty = content_dirty.len(),
                    static_dirty_count = static_dirty.len(),
                    use_static_split,
                    "tile frame"
                );
            }

            // When there are video tiles, let H.264 through for the video
            // region. Otherwise tiles handle the full screen.
            let cdp_has_video = cdp_video_region_hint.is_some() && scroll_cooldown_frames == 0;
            let _cdp_has_motion =
                cdp_has_video && cdp_motion_tiles >= MIN_CHANGED_VIDEO_TILES_FOR_H264;
            let has_video = cdp_has_video;
            let desired_h264 = match h264_mode_for_tile {
                H264Mode::Always => true,
                H264Mode::VideoTiles => has_video,
                H264Mode::Off => false,
            };

            let next_capture_region = if matches!(h264_mode_for_tile, H264Mode::VideoTiles) {
                cdp_has_video.then_some(cdp_video_region_hint).flatten()
            } else {
                None
            };
            if next_capture_region == pending_capture_region {
                pending_capture_region_streak = pending_capture_region_streak.saturating_add(1);
            } else {
                pending_capture_region = next_capture_region;
                pending_capture_region_streak = 1;
            }
            let committed_capture_region = if ffmpeg_capture_region.is_none()
                || pending_capture_region_streak >= REGION_RECONFIG_STABLE_FRAMES
            {
                pending_capture_region
            } else {
                ffmpeg_capture_region
            };

            if desired_h264 && committed_capture_region != ffmpeg_capture_region {
                // Preload region before enabling H.264 so FFmpeg starts directly
                // on the ROI instead of a transient full-frame capture.
                let mut allow_reconfig = true;
                if let (Some(old), Some(new)) = (ffmpeg_capture_region, committed_capture_region) {
                    let small_jitter = old.x.abs_diff(new.x) <= 64
                        && old.y.abs_diff(new.y) <= 64
                        && old.w.abs_diff(new.w) <= 128
                        && old.h.abs_diff(new.h) <= 128;
                    let too_soon = now.duration_since(last_region_reconfig_at)
                        < std::time::Duration::from_millis(REGION_RECONFIG_MIN_INTERVAL_MS);
                    if small_jitter && too_soon {
                        allow_reconfig = false;
                    }
                }
                if allow_reconfig {
                    ffmpeg_capture_region = committed_capture_region;
                    last_region_reconfig_at = now;
                    let _ = cmd_tx_for_tile.send(capture::ffmpeg::PipelineCmd::SetRegion(
                        committed_capture_region,
                    ));
                }
            } else if !desired_h264 {
                // Skip SetRegion while H.264 is disabled to avoid a pointless
                // restart when transitioning from video->no-video.
                ffmpeg_capture_region = committed_capture_region;
            }

            let next_tile_info = if matches!(h264_mode_for_tile, H264Mode::VideoTiles) {
                ffmpeg_capture_region.map(|region| VideoTileInfo {
                    tile_x: region.x as u16,
                    tile_y: region.y as u16,
                    tile_w: region.w as u16,
                    tile_h: region.h as u16,
                    screen_w,
                    screen_h,
                })
            } else {
                None
            };
            {
                let mut guard = match video_tile_info_for_tile.lock() {
                    Ok(g) => g,
                    Err(poisoned) => poisoned.into_inner(),
                };
                *guard = next_tile_info;
            }

            let mut effective_h264 = desired_h264;
            if h264_enabled
                && !desired_h264
                && now.duration_since(last_h264_toggle_at)
                    < std::time::Duration::from_millis(H264_MIN_ON_DURATION_MS)
            {
                effective_h264 = true;
            }
            if effective_h264 != h264_enabled {
                h264_enabled = effective_h264;
                last_h264_toggle_at = now;
                let _ =
                    cmd_tx_for_tile.send(capture::ffmpeg::PipelineCmd::SetEnabled(effective_h264));
            }
            let tiles_cover_screen = if matches!(h264_mode_for_tile, H264Mode::Off) {
                true
            } else {
                !effective_h264 || ffmpeg_capture_region.is_none()
            };
            tiles_active_for_tile.store(tiles_cover_screen, std::sync::atomic::Ordering::Relaxed);

            // Send all tile data (content + static) BEFORE BatchEnd so the
            // client processes everything in a single batch.  Previously,
            // static tiles were sent after BatchEnd, causing them to be
            // deferred to the next batch — leaving the buffer row (header/
            // content seam) black for one frame during scroll.
            //
            // Order: content tiles (sans BatchEnd) → static tiles → BatchEnd.
            let mut content_frames = result.tile_frames;
            let batch_end_frame = content_frames.pop(); // always BatchEnd
            for frame in content_frames {
                if tile_tx.blocking_send(frame).is_err() {
                    return;
                }
            }
            for frame in static_frames {
                if tile_tx.blocking_send(frame).is_err() {
                    return;
                }
            }
            if let Some(be) = batch_end_frame {
                if tile_tx.blocking_send(be).is_err() {
                    return;
                }
            }

            if let Some(dt) = damage.as_mut() {
                dt.reset();
            }

            // Keep current frame as previous without cloning.
            prev_frame = Some(rgba);
        }
    });

    // Forward tile frames to the gateway
    let tile_gateway = to_gateway.clone();
    let tile_fwd = tokio::spawn(async move {
        while let Some(frame) = tile_rx.recv().await {
            if tile_gateway.send(frame).await.is_err() {
                break;
            }
        }
    });

    let mut ping_seq: u32 = 0;
    let mut ping_timer = tokio::time::interval(Duration::from_secs(5));
    let mut last_mouse_pos: Option<(u16, u16)> = None;
    let mut cdp_text_keyups: HashSet<(u32, u32)> = HashSet::new();
    let mut file_transfers = filetransfer::FileTransferState::from_env().await?;
    let download_watch_task = filetransfer::spawn_download_watcher(
        file_transfers.download_dir().to_path_buf(),
        to_gateway.clone(),
    );
    info!(
        upload_dir = %file_transfers.upload_dir().display(),
        download_dir = %file_transfers.download_dir().display(),
        "file transfer enabled"
    );

    let result = loop {
        tokio::select! {
            _ = ping_timer.tick() => {
                ping_seq += 1;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let _ = to_gateway.send(ControlMessage::Ping { seq: ping_seq, timestamp_ms: now }.to_frame()).await;
            }

            msg = from_gateway.recv() => {
                match msg {
                    Some(frame) => {
                        match Message::from_frame(&frame) {
                            Ok(Message::Control(ctrl)) => {
                                handle_control_ffmpeg(ctrl, &to_gateway, &cmd_tx).await;
                            }
                            Ok(Message::Input(input_msg)) => {
                                // Forward scroll events to tile thread for displacement detection.
                                let mut injected_via_cdp = false;
                                match &input_msg {
                                    bpane_protocol::InputMessage::MouseMove { x, y } => {
                                        last_mouse_pos = Some((*x, *y));
                                    }
                                    bpane_protocol::InputMessage::MouseButton { button, down, x, y } => {
                                        last_mouse_pos = Some((*x, *y));
                                        if *down && *button == 0 {
                                            let _ = video_click_tx.send((*x, *y, std::time::Instant::now()));
                                        }
                                    }
                                    bpane_protocol::InputMessage::MouseScroll { dx, dy } => {
                                        let _ = scroll_tx.send((*dx, *dy));
                                        if chromium_wheel_step_px > 0 {
                                            if let Some(cdp_handle) = browser_video_hint_task.as_ref() {
                                                injected_via_cdp = cdp_handle
                                                    .dispatch_quantized_wheel(
                                                        last_mouse_pos,
                                                        *dx,
                                                        *dy,
                                                        chromium_wheel_step_px,
                                                    )
                                                    .await;
                                                if !injected_via_cdp {
                                                    trace!(
                                                        dx = *dx,
                                                        dy = *dy,
                                                        chromium_wheel_step_px,
                                                        "cdp wheel dispatch unavailable; falling back to XTEST"
                                                    );
                                                }
                                            }
                                        }
                                    }
                                    bpane_protocol::InputMessage::KeyEvent { down, .. } => {
                                        if *down {
                                            input_activity_ms.store(
                                                unix_time_ms_now(),
                                                std::sync::atomic::Ordering::Relaxed,
                                            );
                                            let _ = text_input_tx.send(std::time::Instant::now());
                                        }
                                    }
                                    bpane_protocol::InputMessage::KeyEventEx { down, .. } => {
                                        if *down {
                                            input_activity_ms.store(
                                                unix_time_ms_now(),
                                                std::sync::atomic::Ordering::Relaxed,
                                            );
                                            let _ = text_input_tx.send(std::time::Instant::now());
                                        }
                                    }
                                }
                                if let (
                                    Some(cdp_handle),
                                    bpane_protocol::InputMessage::KeyEventEx {
                                        keycode,
                                        down,
                                        modifiers,
                                        key_char,
                                    },
                                ) = (browser_video_hint_task.as_ref(), &input_msg)
                                {
                                    let key_id = (*keycode, *key_char);
                                    if *down {
                                        if let Some(text) = cdp_insert_text_payload(*modifiers, *key_char) {
                                            injected_via_cdp = cdp_handle.dispatch_text(text).await;
                                            if injected_via_cdp {
                                                cdp_text_keyups.insert(key_id);
                                            }
                                        }
                                    } else if cdp_text_keyups.remove(&key_id) {
                                        injected_via_cdp = true;
                                    }
                                }
                                if injected_via_cdp {
                                    trace!("input injected via cdp: {:?}", input_msg);
                                } else if let Err(e) = input_backend.inject(&input_msg) {
                                    warn!("input inject failed: {e}");
                                } else {
                                    trace!("input injected: {:?}", input_msg);
                                }
                            }
                            Ok(Message::Tiles(tile_msg)) => {
                                if let TileMessage::CacheMiss { frame_seq, col, row, hash } = tile_msg {
                                    let _ = cache_miss_tx.send((frame_seq, col, row, hash));
                                }
                            }
                            Ok(Message::Clipboard(ClipboardMessage::Text { content })) => {
                                clipboard::set_clipboard(display, &content);
                            }
                            Ok(Message::AudioIn(payload)) => {
                                if mic_input.is_none() {
                                    match audio::input::MicInput::new() {
                                        Ok(mic) => {
                                            info!("microphone input: started");
                                            mic_input = Some(mic);
                                        }
                                        Err(e) => {
                                            warn!("microphone input: unavailable ({e})");
                                        }
                                    }
                                }
                                if let Some(ref mut mic) = mic_input {
                                    if let Ok(af) = AudioFrame::decode(&payload) {
                                        mic.write_frame(&af);
                                    }
                                }
                            }
                            Ok(Message::VideoIn(payload)) => {
                                if payload.is_empty() {
                                    camera_input = None;
                                    info!("camera input: stopped");
                                } else {
                                    if camera_input.is_none() {
                                        match camera::CameraInput::new() {
                                            Ok(camera) => {
                                                info!("camera input: started");
                                                camera_input = Some(camera);
                                            }
                                            Err(e) => {
                                                warn!("camera input: unavailable ({e})");
                                            }
                                        }
                                    }
                                    if let Some(ref mut camera) = camera_input {
                                        if let Err(e) = camera.write_frame(&payload) {
                                            warn!("camera input: write failed ({e})");
                                            camera_input = None;
                                        }
                                    }
                                }
                            }
                            Ok(Message::FileUp(file_msg)) => {
                                if let Err(e) = file_transfers.handle_upload_message(file_msg).await {
                                    warn!("file upload handling failed: {e}");
                                }
                            }
                            Ok(_) => {}
                            Err(e) => warn!("failed to decode message: {e}"),
                        }
                    }
                    None => break Ok(()),
                }
            }
        }
    };

    // Stop the pipeline
    let _ = cmd_tx.send(capture::ffmpeg::PipelineCmd::Stop);
    bridge.abort();
    video_fwd.abort();
    tile_thread.abort();
    tile_fwd.abort();
    cursor_task.abort();
    clipboard_task.abort();
    download_watch_task.abort();
    if let Some(task) = audio_task {
        task.abort();
    }
    if let Some(handle) = browser_video_hint_task {
        handle.task.abort();
    }
    result
}

async fn handle_control_ffmpeg(
    msg: ControlMessage,
    to_gateway: &mpsc::Sender<Frame>,
    cmd_tx: &std::sync::mpsc::Sender<capture::ffmpeg::PipelineCmd>,
) {
    match msg {
        ControlMessage::ResolutionRequest { width, height } => {
            debug!("resize request: {}x{}", width, height);
            let (ack_tx, ack_rx) = tokio::sync::oneshot::channel();
            let _ = cmd_tx.send(capture::ffmpeg::PipelineCmd::Resize(
                width as u32,
                height as u32,
                ack_tx,
            ));
            // Wait for the pipeline thread to complete the resize before sending ack
            if let Ok((actual_w, actual_h)) = ack_rx.await {
                let _ = cdp_video::resize_visible_target_window(actual_w, actual_h).await;
                let ack = ControlMessage::ResolutionAck {
                    width: actual_w,
                    height: actual_h,
                };
                let _ = to_gateway.send(ack.to_frame()).await;
            }
        }
        ControlMessage::Ping { seq, timestamp_ms } => {
            let _ = to_gateway
                .send(ControlMessage::Pong { seq, timestamp_ms }.to_frame())
                .await;
        }
        ControlMessage::KeyboardLayoutInfo { layout_hint } => {
            let hint = core::str::from_utf8(&layout_hint)
                .unwrap_or("")
                .trim_end_matches('\0');
            if !hint.is_empty() {
                debug!("client keyboard layout hint: {hint}");
            }
        }
        ControlMessage::BitrateHint { target_bps } => {
            debug!("bitrate hint from gateway: {target_bps} bps");
            let _ = cmd_tx.send(capture::ffmpeg::PipelineCmd::BitrateHint(target_bps));
        }
        _ => {}
    }
}

/// Fallback session using test backends (non-Linux or no display).
async fn run_test_session(
    args: &Args,
    mut from_gateway: mpsc::Receiver<Frame>,
    to_gateway: mpsc::Sender<Frame>,
) -> anyhow::Result<()> {
    use capture::{CaptureBackend, TestCaptureBackend};
    use encode::{EncodeBackend, TestEncoder};

    let mut capture = TestCaptureBackend::new(args.width, args.height);
    let mut encoder = TestEncoder::new(args.width, args.height);
    let mut input_backend: Box<dyn InputBackend> = Box::new(TestInputBackend::default());
    let mut resize_handler = resize::ResizeHandler::new(args.width, args.height);
    let mut nal_id: u32 = 0;
    let frame_interval = Duration::from_micros(1_000_000 / args.fps as u64);

    let (video_tx, mut video_rx) = mpsc::channel::<Frame>(64);
    let video_gateway = to_gateway.clone();
    let video_task = tokio::spawn(async move {
        while let Some(frame) = video_rx.recv().await {
            if video_gateway.send(frame).await.is_err() {
                break;
            }
        }
    });

    let mut frame_timer = tokio::time::interval(frame_interval);
    let mut ping_seq: u32 = 0;
    let mut ping_timer = tokio::time::interval(Duration::from_secs(5));

    let result = loop {
        tokio::select! {
            _ = frame_timer.tick() => {
                if let Ok(Some(raw_frame)) = capture.capture_frame() {
                    if let Ok(encoded) = encoder.encode_frame(&raw_frame) {
                        nal_id += 1;
                        let frags = VideoDatagram::fragment(
                            nal_id, encoded.is_keyframe, encoded.pts_us,
                            &encoded.data, video_datagram_max_fragment_size(None),
                        );
                        for frag in &frags {
                            let _ = video_tx.try_send(
                                Frame::new(ChannelId::Video, frag.encode()),
                            );
                        }
                    }
                }
            }

            _ = ping_timer.tick() => {
                ping_seq += 1;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let _ = to_gateway.send(ControlMessage::Ping { seq: ping_seq, timestamp_ms: now }.to_frame()).await;
            }

            msg = from_gateway.recv() => {
                match msg {
                    Some(frame) => {
                        if let Ok(Message::Control(ctrl)) = Message::from_frame(&frame) {
                            handle_control_test(ctrl, &to_gateway, &mut resize_handler, &mut capture, &mut encoder).await;
                        } else if let Ok(Message::Input(input_msg)) = Message::from_frame(&frame) {
                            let _ = input_backend.inject(&input_msg);
                        }
                    }
                    None => break Ok(()),
                }
            }
        }
    };

    drop(video_tx);
    let _ = video_task.await;
    result
}

async fn handle_control_test(
    msg: ControlMessage,
    to_gateway: &mpsc::Sender<Frame>,
    resize_handler: &mut resize::ResizeHandler,
    capture: &mut dyn capture::CaptureBackend,
    encoder: &mut dyn encode::EncodeBackend,
) {
    match msg {
        ControlMessage::ResolutionRequest { width, height } => {
            let _ = resize_handler.apply(width as u32, height as u32, capture, encoder);
            let ack = ControlMessage::ResolutionAck { width, height };
            let _ = to_gateway.send(ack.to_frame()).await;
        }
        ControlMessage::Ping { seq, timestamp_ms } => {
            let _ = to_gateway
                .send(ControlMessage::Pong { seq, timestamp_ms }.to_frame())
                .await;
        }
        _ => {}
    }
}

// env_*, tile_*_from_env, H264Mode moved to config module.

// Geometry helpers moved to region module.

// TileMotionFeatures, compute_tile_motion_features, bbox_iou,
// bbox_center_shift, is_photo_like_tile moved to video_classify module.

/// Compute the same tile extraction rect used by the tile emitter when a
/// vertical grid offset is active.
fn offset_tile_rect_for_emit(
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

/// Returns true when `current` tile pixels equal `previous` shifted by scroll dy.
/// A mismatch or out-of-bounds mapping marks the tile as residual-dirty.
fn tile_matches_shifted_prev(
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
///
/// For each emit tile, compare current pixels against previous-frame pixels
/// shifted by `scroll_dy`; only mismatches are emitted.
fn build_scroll_residual_emit_coords(
    current: &[u8],
    previous: &[u8],
    stride: usize,
    grid: &tiles::TileGrid,
    grid_offset_y: u16,
    scroll_dy: i16,
    emit_coords: &[tiles::TileCoord],
) -> Vec<tiles::TileCoord> {
    if scroll_dy == 0 || emit_coords.is_empty() {
        return emit_coords.to_vec();
    }
    let screen_h = grid.screen_h as usize;
    let mut out = Vec::with_capacity(emit_coords.len() / 2);

    for &coord in emit_coords {
        let rect = offset_tile_rect_for_emit(coord, grid, grid_offset_y);
        if rect.w == 0 || rect.h == 0 {
            continue;
        }
        if !tile_matches_shifted_prev(current, previous, stride, screen_h, &rect, scroll_dy) {
            out.push(coord);
        }
    }

    out
}

/// Build the exposed-strip dirty set for a vertical scroll copy.
///
/// Returns only tiles that map partially/fully out of previous-frame bounds
/// when shifted by `scroll_dy` (i.e. newly exposed top/bottom strip).
fn build_scroll_exposed_strip_emit_coords(
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

/// Skip speculative canvas shifts when residual analysis already says the frame
/// is effectively a full repaint. In that case, ScrollCopy only creates a wrong
/// intermediate image and the tiles immediately overwrite it.
fn should_emit_scroll_copy(
    scroll_residual_fallback_full: bool,
    scroll_saved_tiles: Option<usize>,
) -> bool {
    if scroll_residual_fallback_full {
        return false;
    }
    scroll_saved_tiles.unwrap_or(1) > 0
}

fn content_scroll_search_limit_px(cdp_scroll_dy_px: Option<i16>) -> usize {
    let cdp_abs = cdp_scroll_dy_px
        .map(|dy| (dy as i32).unsigned_abs() as usize)
        .unwrap_or(0);
    cdp_abs.clamp(256, CONTENT_SCROLL_SEARCH_MAX_PX)
}

fn select_capture_frame_interval(
    base_frame_interval: std::time::Duration,
    scroll_active_frame_interval: std::time::Duration,
    scroll_active_capture_frames_remaining: u8,
) -> std::time::Duration {
    if scroll_active_capture_frames_remaining > 0 {
        scroll_active_frame_interval.min(base_frame_interval)
    } else {
        base_frame_interval
    }
}

fn next_scroll_active_capture_frames(
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

fn should_defer_scroll_repair(
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

fn select_wheel_trusted_scroll(
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

    if let Some((
        content_dy,
        content_confidence,
        content_direction_matches,
        content_min_confidence,
    )) = content_scroll
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

fn is_scroll_delta_quantized(scroll_dy: i16, quantum_px: u16) -> bool {
    if quantum_px == 0 {
        return true;
    }
    let dy = i32::from(scroll_dy).unsigned_abs();
    dy % u32::from(quantum_px) == 0
}

fn can_emit_scroll_copy(scroll_dy: i16, quantum_px: u16, tile_size: u16) -> bool {
    if tile_size == 0 {
        return false;
    }
    let dy = i32::from(scroll_dy).unsigned_abs();
    dy != 0 && is_scroll_delta_quantized(scroll_dy, quantum_px) && dy % u32::from(tile_size) == 0
}

fn has_scroll_region_split(
    region_top: u16,
    region_bottom: u16,
    region_right: u16,
    screen_h: u16,
    screen_w: u16,
) -> bool {
    region_top > 0 || region_bottom < screen_h || region_right < screen_w
}

fn is_content_tile_in_scroll_region(
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

// hash_tile_region moved to region module.

/// Detect vertical scroll displacement by comparing pixel columns between frames.
///
/// More reliable than row-hash comparison because vertical columns pass through
/// diverse content regions (headers, text, footers), reducing false positives
/// from uniform horizontal bands (white backgrounds).
///
/// Samples 5 columns at [10%, 30%, 50%, 70%, 90%] of screen width.
/// For each candidate dy, counts matching pixels across all columns.
/// Returns the displacement if ≥3 of 5 columns agree.
fn detect_column_scroll(
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
    // Sample 5 columns at different x positions for diversity
    let sample_xs: Vec<usize> = [10, 30, 50, 70, 90]
        .iter()
        .map(|pct| (*pct * width / 100).min(width - 1))
        .collect();

    let mut best_dy: i32 = 0;
    let mut best_total_matches: usize = 0;
    let mut best_agreeing_cols: usize = 0;

    for dy in 1..=max_dy {
        // Try both positive and negative displacement
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
                        (y, y + dy) // content moved up: curr[y] was prev[y+dy]
                    } else {
                        (y + dy, y) // content moved down: curr[y+dy] was prev[y]
                    };

                    let curr_off = curr_y * stride + byte_offset;
                    let prev_off = prev_y * stride + byte_offset;

                    // Compare RGBA pixel (4 bytes)
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

// preflight_checks moved to config module.

#[cfg(test)]
mod tests {
    use super::*;
    use capture::TestCaptureBackend;
    use encode::TestEncoder;

    #[test]
    fn damage_gate_stays_on_for_full_frame_video() {
        assert!(should_gate_video_delta_on_damage(None));
    }

    #[test]
    fn damage_gate_is_bypassed_for_video_tiles() {
        let tile = VideoTileInfo {
            tile_x: 10,
            tile_y: 20,
            tile_w: 640,
            tile_h: 360,
            screen_w: 1280,
            screen_h: 720,
        };
        assert!(!should_gate_video_delta_on_damage(Some(tile)));
    }

    #[test]
    fn video_datagram_fragment_budget_stays_below_safe_payload_without_tile_info() {
        let max_frag = video_datagram_max_fragment_size(None);
        let encoded = VideoDatagram::fragment(1, false, 0, &vec![0; max_frag], max_frag);
        assert_eq!(encoded.len(), 1);
        assert!(encoded[0].encode().len() <= SAFE_VIDEO_DATAGRAM_PAYLOAD);
    }

    #[test]
    fn video_datagram_fragment_budget_stays_below_safe_payload_with_tile_info() {
        let tile = VideoTileInfo {
            tile_x: 10,
            tile_y: 20,
            tile_w: 640,
            tile_h: 360,
            screen_w: 1280,
            screen_h: 720,
        };
        let max_frag = video_datagram_max_fragment_size(Some(tile));
        let encoded = VideoDatagram::fragment_with_tile(
            1,
            false,
            0,
            &vec![0; max_frag],
            max_frag,
            Some(tile),
        );
        assert_eq!(encoded.len(), 1);
        assert!(encoded[0].encode().len() <= SAFE_VIDEO_DATAGRAM_PAYLOAD);
    }

    // editable_hint, expand_tile_bounds, cdp_insert_text, extend_dirty tests
    // moved to region::tests.

    #[tokio::test]
    async fn handle_control_test_ping_pong() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut resize = resize::ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        let ping = ControlMessage::Ping {
            seq: 42,
            timestamp_ms: 1000,
        };
        handle_control_test(ping, &tx, &mut resize, &mut capture, &mut encoder).await;

        let response = rx.recv().await.unwrap();
        let msg = ControlMessage::decode(&response.payload).unwrap();
        assert!(matches!(
            msg,
            ControlMessage::Pong {
                seq: 42,
                timestamp_ms: 1000
            }
        ));
    }

    #[tokio::test]
    async fn handle_control_test_resize() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut resize = resize::ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        let req = ControlMessage::ResolutionRequest {
            width: 1920,
            height: 1080,
        };
        handle_control_test(req, &tx, &mut resize, &mut capture, &mut encoder).await;

        let response = rx.recv().await.unwrap();
        let msg = ControlMessage::decode(&response.payload).unwrap();
        assert!(matches!(
            msg,
            ControlMessage::ResolutionAck {
                width: 1920,
                height: 1080
            }
        ));
    }

    #[tokio::test]
    async fn handle_control_test_keyboard_layout_info() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut resize = resize::ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        let mut layout_hint = [0u8; 32];
        layout_hint[..2].copy_from_slice(b"fr");
        let msg = ControlMessage::KeyboardLayoutInfo { layout_hint };
        handle_control_test(msg, &tx, &mut resize, &mut capture, &mut encoder).await;

        // KeyboardLayoutInfo is informational — no response expected
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn handle_control_test_keyboard_layout_info_empty() {
        let (tx, mut rx) = mpsc::channel(16);
        let mut resize = resize::ResizeHandler::new(640, 480);
        let mut capture = TestCaptureBackend::new(640, 480);
        let mut encoder = TestEncoder::new(640, 480);

        let msg = ControlMessage::KeyboardLayoutInfo {
            layout_hint: [0u8; 32],
        };
        handle_control_test(msg, &tx, &mut resize, &mut capture, &mut encoder).await;

        // Empty layout hint — still no crash and no response
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn session_ready_includes_keyboard_layout_flag() {
        let flags = SessionFlags::new(
            SessionFlags::CLIPBOARD | SessionFlags::FILE_TRANSFER | SessionFlags::KEYBOARD_LAYOUT,
        );
        assert!(flags.has(SessionFlags::KEYBOARD_LAYOUT));
        assert!(flags.has(SessionFlags::CLIPBOARD));
        assert!(flags.has(SessionFlags::FILE_TRANSFER));
        assert!(!flags.has(SessionFlags::AUDIO));
    }

    #[tokio::test]
    async fn session_flags_with_audio() {
        // When audio is available, flags should include AUDIO and MICROPHONE
        let mut flags = SessionFlags::new(
            SessionFlags::CLIPBOARD | SessionFlags::FILE_TRANSFER | SessionFlags::KEYBOARD_LAYOUT,
        );
        flags = SessionFlags::new(flags.0 | SessionFlags::AUDIO | SessionFlags::MICROPHONE);
        assert!(flags.has(SessionFlags::AUDIO));
        assert!(flags.has(SessionFlags::MICROPHONE));
        assert!(flags.has(SessionFlags::CLIPBOARD));
        assert!(flags.has(SessionFlags::FILE_TRANSFER));
        assert!(flags.has(SessionFlags::KEYBOARD_LAYOUT));
    }

    #[tokio::test]
    async fn session_flags_without_audio() {
        // When audio is unavailable, flags should NOT include AUDIO or MICROPHONE
        let flags = SessionFlags::new(
            SessionFlags::CLIPBOARD | SessionFlags::FILE_TRANSFER | SessionFlags::KEYBOARD_LAYOUT,
        );
        assert!(!flags.has(SessionFlags::AUDIO));
        assert!(!flags.has(SessionFlags::MICROPHONE));
    }

    #[tokio::test]
    async fn session_ready_audio_flag_encodes_in_wire() {
        // Verify SessionReady with AUDIO flag round-trips through wire encoding
        let flags = SessionFlags::new(
            SessionFlags::AUDIO
                | SessionFlags::CLIPBOARD
                | SessionFlags::FILE_TRANSFER
                | SessionFlags::KEYBOARD_LAYOUT,
        );
        let ready = ControlMessage::SessionReady { version: 2, flags };
        let frame = ready.to_frame();
        let decoded = ControlMessage::decode(&frame.payload).unwrap();
        match decoded {
            ControlMessage::SessionReady {
                version,
                flags: decoded_flags,
            } => {
                assert_eq!(version, 2);
                assert!(decoded_flags.has(SessionFlags::AUDIO));
                assert!(decoded_flags.has(SessionFlags::CLIPBOARD));
            }
            _ => panic!("expected SessionReady"),
        }
    }

    #[test]
    fn has_audio_flag_extraction() {
        let with_audio = SessionFlags::new(
            SessionFlags::AUDIO | SessionFlags::CLIPBOARD | SessionFlags::KEYBOARD_LAYOUT,
        );
        assert!(with_audio.has(SessionFlags::AUDIO));

        let without_audio =
            SessionFlags::new(SessionFlags::CLIPBOARD | SessionFlags::KEYBOARD_LAYOUT);
        assert!(!without_audio.has(SessionFlags::AUDIO));
    }

    #[test]
    fn clipboard_frame_dispatches_to_message_clipboard() {
        let msg = ClipboardMessage::Text {
            content: b"hello from browser".to_vec(),
        };
        let frame = msg.to_frame();
        let decoded = Message::from_frame(&frame).unwrap();
        match decoded {
            Message::Clipboard(ClipboardMessage::Text { content }) => {
                assert_eq!(content, b"hello from browser");
            }
            other => panic!("expected Message::Clipboard, got {:?}", other),
        }
    }

    #[test]
    fn clipboard_frame_empty_text() {
        let msg = ClipboardMessage::Text {
            content: Vec::new(),
        };
        let frame = msg.to_frame();
        let decoded = Message::from_frame(&frame).unwrap();
        assert!(matches!(
            decoded,
            Message::Clipboard(ClipboardMessage::Text { content }) if content.is_empty()
        ));
    }

    // ── Phase 9: BitrateHint forwarding ─────────────────────────────

    #[tokio::test]
    async fn handle_control_ffmpeg_bitrate_hint() {
        let (tx, _rx) = mpsc::channel(16);
        let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();

        let msg = ControlMessage::BitrateHint {
            target_bps: 4_000_000,
        };
        handle_control_ffmpeg(msg, &tx, &cmd_tx).await;

        // Should send BitrateHint command through the pipeline channel
        match cmd_rx.try_recv() {
            Ok(capture::ffmpeg::PipelineCmd::BitrateHint(bps)) => {
                assert_eq!(bps, 4_000_000);
            }
            other => panic!("expected BitrateHint, got {:?}", other.is_ok()),
        }
    }

    // bbox_iou, tile_motion_features tests moved to video_classify::tests.

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

    #[test]
    fn scroll_copy_policy_skips_full_repaints() {
        assert!(!should_emit_scroll_copy(true, Some(12)));
        assert!(!should_emit_scroll_copy(false, Some(0)));
        assert!(should_emit_scroll_copy(false, Some(1)));
        assert!(should_emit_scroll_copy(false, None));
    }

    #[test]
    fn content_scroll_search_limit_tracks_large_cdp_deltas() {
        assert_eq!(content_scroll_search_limit_px(None), 256);
        assert_eq!(content_scroll_search_limit_px(Some(64)), 256);
        assert_eq!(content_scroll_search_limit_px(Some(-320)), 320);
        assert_eq!(content_scroll_search_limit_px(Some(512)), 384);
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

    // css_pixels_scale_to_framebuffer_pixels moved to region::tests.
}
