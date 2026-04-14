//! FFmpeg session orchestration: spawns all subsystem tasks, constructs
//! the tile capture thread, runs the message dispatch loop, and cleans up.

use std::sync::Arc;

use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use bpane_protocol::VideoDatagram;

use crate::audio;
use crate::capture;
use crate::cdp_video;
use crate::clipboard;
use crate::config::{H264Mode, TileCaptureConfig};
use crate::cursor;
use crate::input::{self, InputBackend, TestInputBackend};
use crate::message_dispatch;
use crate::tile_loop;
use crate::VideoTileInfo;

/// Run the production FFmpeg + tile session.
pub async fn run_ffmpeg_session(
    width: u32,
    height: u32,
    fps: u32,
    display: &str,
    has_audio: bool,
    from_gateway: mpsc::Receiver<Frame>,
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

    let cursor_pos = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let cursor_task = cursor::spawn_cursor_task_with_pos(
        display.to_string(),
        to_gateway.clone(),
        Some(cursor_pos.clone()),
    );

    let audio_task = if has_audio {
        Some(audio::spawn_audio_capture(to_gateway.clone()))
    } else {
        None
    };

    let clipboard_task = clipboard::spawn_clipboard_task(display.to_string(), to_gateway.clone());

    let tile_config = TileCaptureConfig::from_env();
    let h264_mode = tile_config.h264_mode;
    info!(?h264_mode, "h264 mode");
    let chromium_wheel_step_px = tile_config.chromium_wheel_step_px;

    // Spawn FFmpeg pipeline
    let (cmd_tx, nal_rx) = capture::ffmpeg::spawn_pipeline(
        display.to_string(),
        width,
        height,
        fps,
        h264_mode.starts_enabled(),
    )?;

    let mut input_backend: Box<dyn InputBackend> = if cfg!(target_os = "linux") {
        match input::mouse::MouseInjector::new()
            .and_then(|m| Ok((m, input::keyboard::KeyboardInjector::new()?)))
        {
            Ok((mouse, keyboard)) => {
                info!("input backend: XTEST (mouse + keyboard)");
                Box::new(input::CombinedInjector { mouse, keyboard })
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

    // Shared state
    let tiles_active = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let video_tile_info = Arc::new(std::sync::Mutex::new(None::<VideoTileInfo>));
    let input_activity_ms = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let browser_video_hint = Arc::new(std::sync::Mutex::new(cdp_video::PageHintState::default()));
    let browser_video_hint_task =
        if matches!(h264_mode, H264Mode::VideoTiles) || chromium_wheel_step_px > 0 {
            Some(cdp_video::spawn_video_hint_task(browser_video_hint.clone()))
        } else {
            None
        };

    // NAL bridge: FFmpeg → gateway
    let mut nal_id: u32 = 0;
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
                    Err(p) => p.into_inner(),
                };
                *guard
            };
            if !encoded.is_keyframe && super::should_gate_video_delta_on_damage(tile_info) {
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
                super::video_datagram_max_fragment_size(tile_info),
                tile_info,
            );
            for frag in &fragments {
                if video_tx
                    .blocking_send(Frame::new(ChannelId::Video, frag.encode()))
                    .is_err()
                {
                    return;
                }
            }
            if let Some(dt) = damage.as_mut() {
                dt.reset();
            }
        }
        debug!("bridge: NAL receive channel closed");
    });

    // Video frame forwarder
    let video_gateway = to_gateway.clone();
    let video_fwd = tokio::spawn(async move {
        while let Some(frame) = video_rx.recv().await {
            if video_gateway.send(frame).await.is_err() {
                break;
            }
        }
    });

    // Event channels to tile thread
    let (scroll_tx, scroll_rx) = std::sync::mpsc::channel::<(i16, i16)>();
    let (text_input_tx, text_input_rx) = std::sync::mpsc::channel::<std::time::Instant>();
    let (cache_miss_tx, cache_miss_rx) = std::sync::mpsc::channel::<(u32, u16, u16, u64)>();

    // Tile capture thread
    let (tile_tx, mut tile_rx) = mpsc::channel::<Frame>(256);
    let display_for_tiles = display.to_string();
    let cmd_tx_for_tiles = cmd_tx.clone();
    let tiles_active_for_tiles = tiles_active.clone();
    let video_tile_info_for_tiles = video_tile_info.clone();
    let browser_video_hint_for_tiles = browser_video_hint.clone();
    let input_activity_for_tiles = input_activity_ms.clone();
    let tile_config_for_tiles = tile_config.clone();
    let tile_thread = tokio::task::spawn_blocking(move || {
        if let Some(thread) = tile_loop::TileCaptureThread::new(
            &display_for_tiles,
            width,
            height,
            tile_config_for_tiles,
            tile_tx,
            cmd_tx_for_tiles,
            tiles_active_for_tiles,
            video_tile_info_for_tiles,
            browser_video_hint_for_tiles,
            input_activity_for_tiles,
            scroll_rx,
            text_input_rx,
            cache_miss_rx,
        ) {
            thread.run();
        }
    });

    // Tile frame forwarder
    let tile_gateway = to_gateway.clone();
    let tile_fwd = tokio::spawn(async move {
        while let Some(frame) = tile_rx.recv().await {
            if tile_gateway.send(frame).await.is_err() {
                break;
            }
        }
    });

    // Run the message dispatch loop
    let result = message_dispatch::run(
        display,
        from_gateway,
        to_gateway,
        &cmd_tx,
        &mut input_backend,
        &input_activity_ms,
        &browser_video_hint_task,
        chromium_wheel_step_px,
        &scroll_tx,
        &text_input_tx,
        &cache_miss_tx,
    )
    .await;

    // Cleanup
    let _ = cmd_tx.send(capture::ffmpeg::PipelineCmd::Stop);
    bridge.abort();
    video_fwd.abort();
    tile_thread.abort();
    tile_fwd.abort();
    cursor_task.abort();
    clipboard_task.abort();
    if let Some(task) = audio_task {
        task.abort();
    }
    if let Some(handle) = browser_video_hint_task {
        handle.task.abort();
    }
    result
}
