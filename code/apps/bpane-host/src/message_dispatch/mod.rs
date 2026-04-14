//! Gateway message dispatch loop for FFmpeg sessions.
//!
//! Handles the `tokio::select!` loop that processes incoming gateway
//! messages: control, input, clipboard, audio, video, file transfer.

#[cfg(test)]
mod tests;

use std::collections::HashSet;

use tokio::sync::mpsc;
use tracing::{debug, info, trace, warn};

use bpane_protocol::frame::{Frame, Message};
use bpane_protocol::{AudioFrame, ClipboardMessage, ControlMessage, TileMessage};

use crate::audio;
use crate::camera;
use crate::capture;
use crate::cdp_video;
use crate::clipboard;
use crate::filetransfer;
use crate::input::InputBackend;
use crate::region::cdp_insert_text_payload;

/// Run the gateway message dispatch loop for an FFmpeg session.
/// Returns when the gateway disconnects.
#[allow(clippy::too_many_arguments)]
pub async fn run(
    display: &str,
    mut from_gateway: mpsc::Receiver<Frame>,
    to_gateway: mpsc::Sender<Frame>,
    cmd_tx: &std::sync::mpsc::Sender<capture::ffmpeg::PipelineCmd>,
    input_backend: &mut Box<dyn InputBackend>,
    input_activity_ms: &std::sync::Arc<std::sync::atomic::AtomicU64>,
    browser_video_hint_task: &Option<cdp_video::BrowserCdpHandle>,
    chromium_wheel_step_px: u16,
    scroll_tx: &std::sync::mpsc::Sender<(i16, i16)>,
    text_input_tx: &std::sync::mpsc::Sender<std::time::Instant>,
    cache_miss_tx: &std::sync::mpsc::Sender<(u32, u16, u16, u64)>,
) -> anyhow::Result<()> {
    let mut ping_seq: u32 = 0;
    let mut ping_timer = tokio::time::interval(std::time::Duration::from_secs(5));
    let mut last_mouse_pos: Option<(u16, u16)> = None;
    let mut cdp_text_keyups: HashSet<(u32, u32)> = HashSet::new();
    let mut mic_input: Option<audio::input::MicInput> = None;
    let mut camera_input: Option<camera::CameraInput> = None;
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
                                handle_control_ffmpeg(ctrl, &to_gateway, cmd_tx).await;
                            }
                            Ok(Message::Input(input_msg)) => {
                                let mut injected_via_cdp = false;
                                match &input_msg {
                                    bpane_protocol::InputMessage::MouseMove { x, y } => {
                                        last_mouse_pos = Some((*x, *y));
                                    }
                                    bpane_protocol::InputMessage::MouseButton { x, y, .. } => {
                                        last_mouse_pos = Some((*x, *y));
                                    }
                                    bpane_protocol::InputMessage::MouseScroll { dx, dy } => {
                                        let _ = scroll_tx.send((*dx, *dy));
                                        if chromium_wheel_step_px > 0 {
                                            if let Some(cdp_handle) = browser_video_hint_task.as_ref() {
                                                injected_via_cdp = cdp_handle
                                                    .dispatch_quantized_wheel(last_mouse_pos, *dx, *dy, chromium_wheel_step_px)
                                                    .await;
                                                if !injected_via_cdp {
                                                    trace!(dx = *dx, dy = *dy, chromium_wheel_step_px, "cdp wheel dispatch unavailable; falling back to XTEST");
                                                }
                                            }
                                        }
                                    }
                                    bpane_protocol::InputMessage::KeyEvent { down, .. } => {
                                        if *down {
                                            input_activity_ms.store(super::unix_time_ms_now(), std::sync::atomic::Ordering::Relaxed);
                                            let _ = text_input_tx.send(std::time::Instant::now());
                                        }
                                    }
                                    bpane_protocol::InputMessage::KeyEventEx { down, .. } => {
                                        if *down {
                                            input_activity_ms.store(super::unix_time_ms_now(), std::sync::atomic::Ordering::Relaxed);
                                            let _ = text_input_tx.send(std::time::Instant::now());
                                        }
                                    }
                                }
                                if let (
                                    Some(cdp_handle),
                                    bpane_protocol::InputMessage::KeyEventEx { keycode, down, modifiers, key_char },
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
                                        Ok(mic) => { info!("microphone input: started"); mic_input = Some(mic); }
                                        Err(e) => { warn!("microphone input: unavailable ({e})"); }
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
                                            Ok(camera) => { info!("camera input: started"); camera_input = Some(camera); }
                                            Err(e) => { warn!("camera input: unavailable ({e})"); }
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

    download_watch_task.abort();
    result
}

/// Handle control messages in FFmpeg session mode.
pub async fn handle_control_ffmpeg(
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
