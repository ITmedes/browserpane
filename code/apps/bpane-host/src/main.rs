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
mod scroll;
pub mod tiles;
mod tile_loop;
mod video_classify;
mod video_region;

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

// Scroll constants moved to scroll module.
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
use scroll::{
    build_scroll_exposed_strip_emit_coords, build_scroll_residual_emit_coords,
    can_emit_scroll_copy, content_scroll_search_limit_px, detect_column_scroll,
    has_scroll_region_split, is_content_tile_in_scroll_region, is_scroll_delta_quantized,
    next_scroll_active_capture_frames, offset_tile_rect_for_emit, select_capture_frame_interval,
    select_wheel_trusted_scroll, should_defer_scroll_repair, should_emit_scroll_copy,
    tile_matches_shifted_prev,
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
    let (tile_tx, mut tile_rx) = mpsc::channel::<Frame>(256);
    let display_for_tiles = display.to_string();
    let tile_init_w = args.width;
    let tile_init_h = args.height;
    let cmd_tx_for_tiles = cmd_tx.clone();
    let tiles_active_for_tiles = tiles_active.clone();
    let video_tile_info_for_tiles = video_tile_info.clone();
    let browser_video_hint_for_tiles = browser_video_hint.clone();
    let input_activity_for_tiles = input_activity_ms.clone();
    let tile_thread = tokio::task::spawn_blocking(move || {
        if let Some(thread) = tile_loop::TileCaptureThread::new(
            &display_for_tiles,
            tile_init_w,
            tile_init_h,
            h264_mode,
            tile_size_from_env(),
            tile_codec_from_env(),
            scroll_copy_quantum_px,
            std::time::Duration::from_millis(100),
            std::time::Duration::from_millis(
                env_u32_clamped("BPANE_SCROLL_ACTIVE_FRAME_INTERVAL_MS", 33, 16, 100) as u64,
            ),
            env_u32_clamped("BPANE_SCROLL_ACTIVE_CAPTURE_FRAMES", 8, 0, 32) as u8,
            env_u32_clamped("BPANE_CDP_MIN_VIDEO_WIDTH", 320, 2, 4096) & !1,
            env_u32_clamped("BPANE_CDP_MIN_VIDEO_HEIGHT", 180, 2, 4096) & !1,
            env_f32_clamped("BPANE_CDP_MIN_VIDEO_AREA_RATIO", 0.08, 0.01, 0.95),
            env_u32_clamped("BPANE_VIDEO_CLICK_ARM_MS", 8_000, 250, 60_000) as u64,
            env_bool("BPANE_SCROLL_THIN_MODE", false),
            tile_tx,
            cmd_tx_for_tiles,
            tiles_active_for_tiles,
            video_tile_info_for_tiles,
            browser_video_hint_for_tiles,
            input_activity_for_tiles,
            scroll_rx,
            video_click_rx,
            text_input_rx,
            cache_miss_rx,
        ) {
            thread.run();
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

// Scroll functions moved to scroll module.
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

    // Scroll tests moved to scroll::tests.
    // bbox/motion tests moved to video_classify::tests.
    // Geometry tests moved to region::tests.
}
