use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc as std_mpsc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::{env, str::FromStr};

use crate::encode::EncodedFrame;

/// Pixel-space capture region for x11grab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaptureRegion {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

/// Command sent to the FFmpeg pipeline thread.
pub enum PipelineCmd {
    /// Resize to (width, height). Sends the actual applied (w, h) back via the oneshot.
    Resize(u32, u32, tokio::sync::oneshot::Sender<(u16, u16)>),
    BitrateHint(u32),
    /// Enable/disable the H.264 encoder process while keeping the pipeline
    /// control thread alive for resize and future re-enable.
    SetEnabled(bool),
    /// Restrict x11grab capture to a screen sub-rectangle.
    /// `None` restores full-screen capture.
    SetRegion(Option<CaptureRegion>),
    Stop,
}

/// Runs the FFmpeg capture+encode pipeline in a dedicated thread.
/// Communicates via channels: receives resize commands, sends encoded NAL units.
pub fn spawn_pipeline(
    display: String,
    initial_w: u32,
    initial_h: u32,
    fps: u32,
    start_enabled: bool,
) -> anyhow::Result<(
    std_mpsc::Sender<PipelineCmd>,
    std_mpsc::Receiver<EncodedFrame>,
)> {
    let (cmd_tx, cmd_rx) = std_mpsc::channel::<PipelineCmd>();
    let (nal_tx, nal_rx) = std_mpsc::sync_channel::<EncodedFrame>(64);

    std::thread::Builder::new()
        .name("ffmpeg-pipeline".into())
        .spawn(move || {
            pipeline_thread(
                display,
                initial_w,
                initial_h,
                fps,
                start_enabled,
                cmd_rx,
                nal_tx,
            );
        })?;

    Ok((cmd_tx, nal_rx))
}

fn pipeline_thread(
    display: String,
    mut width: u32,
    mut height: u32,
    fps: u32,
    mut pipeline_enabled: bool,
    cmd_rx: std_mpsc::Receiver<PipelineCmd>,
    nal_tx: std_mpsc::SyncSender<EncodedFrame>,
) {
    let mut child: Option<Child> = None;
    let mut nal_buf = Vec::with_capacity(256 * 1024);
    let mut read_buf = vec![0u8; 128 * 1024];
    let mut bitrate_override: Option<String> = None;
    let mut maxrate_override: Option<String> = None;
    let mut capture_region: Option<CaptureRegion> = None;

    // Ensure even dimensions for H.264
    width &= !1;
    height &= !1;
    let mut active_capture = effective_capture_region(width, height, capture_region);
    if pipeline_enabled {
        child = start_ffmpeg(
            &display,
            width,
            height,
            capture_region,
            fps,
            bitrate_override.as_deref(),
            maxrate_override.as_deref(),
        );
    }

    loop {
        // Check for commands (non-blocking)
        match cmd_rx.try_recv() {
            Ok(PipelineCmd::Resize(w, h, ack_tx)) => {
                // H.264 requires even dimensions
                let w = (w.max(320).min(3840)) & !1;
                let h = (h.max(200).min(2160)) & !1;
                if w != width || h != height {
                    tracing::debug!(from = %format!("{width}x{height}"), to = %format!("{w}x{h}"), "pipeline resize");
                    stop_ffmpeg(&mut child);
                    resize_display(&display, w, h);
                    width = w;
                    height = h;
                    capture_region = capture_region
                        .and_then(|region| clamp_capture_region(region, width, height));
                    active_capture = effective_capture_region(width, height, capture_region);
                    nal_buf.clear();
                    if pipeline_enabled {
                        child = start_ffmpeg(
                            &display,
                            width,
                            height,
                            capture_region,
                            fps,
                            bitrate_override.as_deref(),
                            maxrate_override.as_deref(),
                        );
                    }
                }
                // Notify caller of actual applied dimensions
                let _ = ack_tx.send((w as u16, h as u16));
            }
            Ok(PipelineCmd::BitrateHint(target_bps)) => {
                tracing::debug!(
                    target_bps,
                    "pipeline bitrate hint (stored for next restart)"
                );
                bitrate_override = Some(format!("{}k", target_bps / 1000));
                maxrate_override = Some(format!("{}k", target_bps * 2 / 1000));
                // Don't restart FFmpeg just for a bitrate change — it causes
                // visible glitches. The new bitrate will take effect on the
                // next resize or natural restart.
            }
            Ok(PipelineCmd::SetEnabled(enabled)) => {
                if enabled != pipeline_enabled {
                    pipeline_enabled = enabled;
                    if pipeline_enabled {
                        tracing::debug!("pipeline enabled");
                        active_capture = effective_capture_region(width, height, capture_region);
                        nal_buf.clear();
                        child = start_ffmpeg(
                            &display,
                            width,
                            height,
                            capture_region,
                            fps,
                            bitrate_override.as_deref(),
                            maxrate_override.as_deref(),
                        );
                    } else {
                        tracing::debug!("pipeline disabled");
                        stop_ffmpeg(&mut child);
                        nal_buf.clear();
                    }
                }
            }
            Ok(PipelineCmd::SetRegion(region)) => {
                let normalized = region.and_then(|r| clamp_capture_region(r, width, height));
                if normalized != capture_region {
                    tracing::debug!(
                        old = ?capture_region,
                        new = ?normalized,
                        "pipeline capture region changed"
                    );
                    capture_region = normalized;
                    active_capture = effective_capture_region(width, height, capture_region);
                    if pipeline_enabled {
                        stop_ffmpeg(&mut child);
                        nal_buf.clear();
                        child = start_ffmpeg(
                            &display,
                            width,
                            height,
                            capture_region,
                            fps,
                            bitrate_override.as_deref(),
                            maxrate_override.as_deref(),
                        );
                    }
                }
            }
            Ok(PipelineCmd::Stop) => {
                stop_ffmpeg(&mut child);
                return;
            }
            Err(std_mpsc::TryRecvError::Disconnected) => {
                stop_ffmpeg(&mut child);
                return;
            }
            Err(std_mpsc::TryRecvError::Empty) => {}
        }

        if !pipeline_enabled {
            // Encoder disabled: block on command channel instead of polling.
            match cmd_rx.recv_timeout(std::time::Duration::from_millis(200)) {
                Ok(cmd) => {
                    // Re-process command by pushing it back through the loop.
                    // We need to handle it inline since we already consumed it.
                    match cmd {
                        PipelineCmd::Resize(w, h, ack_tx) => {
                            let w = (w.max(320).min(3840)) & !1;
                            let h = (h.max(200).min(2160)) & !1;
                            if w != width || h != height {
                                tracing::debug!(from = %format!("{width}x{height}"), to = %format!("{w}x{h}"), "pipeline resize (disabled)");
                                resize_display(&display, w, h);
                                width = w;
                                height = h;
                                capture_region = capture_region
                                    .and_then(|region| clamp_capture_region(region, width, height));
                                active_capture =
                                    effective_capture_region(width, height, capture_region);
                                nal_buf.clear();
                            }
                            let _ = ack_tx.send((w as u16, h as u16));
                        }
                        PipelineCmd::BitrateHint(target_bps) => {
                            bitrate_override = Some(format!("{}k", target_bps / 1000));
                            maxrate_override = Some(format!("{}k", target_bps * 2 / 1000));
                        }
                        PipelineCmd::SetEnabled(enabled) => {
                            if enabled {
                                pipeline_enabled = true;
                                tracing::debug!("pipeline enabled");
                                active_capture =
                                    effective_capture_region(width, height, capture_region);
                                nal_buf.clear();
                                child = start_ffmpeg(
                                    &display,
                                    width,
                                    height,
                                    capture_region,
                                    fps,
                                    bitrate_override.as_deref(),
                                    maxrate_override.as_deref(),
                                );
                            }
                        }
                        PipelineCmd::SetRegion(region) => {
                            let normalized =
                                region.and_then(|r| clamp_capture_region(r, width, height));
                            if normalized != capture_region {
                                capture_region = normalized;
                                active_capture =
                                    effective_capture_region(width, height, capture_region);
                            }
                        }
                        PipelineCmd::Stop => {
                            return;
                        }
                    }
                }
                Err(std_mpsc::RecvTimeoutError::Timeout) => {}
                Err(std_mpsc::RecvTimeoutError::Disconnected) => return,
            }
            continue;
        }

        // Read from FFmpeg stdout
        let Some(ref mut proc) = child else {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        };
        let Some(ref mut stdout) = proc.stdout else {
            std::thread::sleep(std::time::Duration::from_millis(100));
            continue;
        };

        match stdout.read(&mut read_buf) {
            Ok(0) => {
                // FFmpeg exited — check stderr and restart
                if let Some(ref mut c) = child {
                    if let Some(ref mut stderr) = c.stderr {
                        let mut err = String::new();
                        let _ = stderr.read_to_string(&mut err);
                        if !err.is_empty() {
                            tracing::error!(stderr = %err.trim(), "ffmpeg stderr");
                        }
                    }
                }
                tracing::warn!("ffmpeg stdout closed, restarting");
                stop_ffmpeg(&mut child);
                nal_buf.clear();
                std::thread::sleep(std::time::Duration::from_millis(500));
                child = start_ffmpeg(
                    &display,
                    width,
                    height,
                    capture_region,
                    fps,
                    bitrate_override.as_deref(),
                    maxrate_override.as_deref(),
                );
            }
            Ok(n) => {
                nal_buf.extend_from_slice(&read_buf[..n]);

                // Extract complete NAL units and send them
                while let Some(nal) = extract_nal(&mut nal_buf) {
                    let nal_type = if nal.len() > 4 { nal[4] & 0x1F } else { 0 };
                    // Mark non-VCL NALs as keyframes so they're never dropped
                    // by the damage gate. SPS(7), PPS(8), SEI(6), and IDR(5)
                    // are all essential for decoder init and must always be sent.
                    let is_keyframe = matches!(nal_type, 5 | 6 | 7 | 8);

                    let ts = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_micros() as u64;

                    let frame = EncodedFrame {
                        data: nal,
                        is_keyframe,
                        pts_us: ts,
                        width: active_capture.w,
                        height: active_capture.h,
                    };

                    if nal_tx.try_send(frame).is_err() {
                        // Receiver dropped or full — session ended or backpressure
                        // Just drop the frame on backpressure
                    }
                }
            }
            Err(e) => {
                tracing::error!("ffmpeg read error: {e}");
                stop_ffmpeg(&mut child);
                std::thread::sleep(std::time::Duration::from_millis(500));
                child = start_ffmpeg(
                    &display,
                    width,
                    height,
                    capture_region,
                    fps,
                    bitrate_override.as_deref(),
                    maxrate_override.as_deref(),
                );
            }
        }
    }
}

fn clamp_capture_region(
    region: CaptureRegion,
    screen_w: u32,
    screen_h: u32,
) -> Option<CaptureRegion> {
    if screen_w < 2 || screen_h < 2 {
        return None;
    }

    let mut x = region.x.min(screen_w.saturating_sub(2));
    let mut y = region.y.min(screen_h.saturating_sub(2));
    x &= !1;
    y &= !1;

    let max_w = screen_w.saturating_sub(x);
    let max_h = screen_h.saturating_sub(y);
    if max_w < 2 || max_h < 2 {
        return None;
    }

    let mut w = region.w.max(2).min(max_w);
    let mut h = region.h.max(2).min(max_h);
    w &= !1;
    h &= !1;
    if w < 2 {
        w = max_w & !1;
    }
    if h < 2 {
        h = max_h & !1;
    }
    if w < 2 || h < 2 {
        return None;
    }

    Some(CaptureRegion { x, y, w, h })
}

fn effective_capture_region(
    screen_w: u32,
    screen_h: u32,
    capture_region: Option<CaptureRegion>,
) -> CaptureRegion {
    if let Some(region) = capture_region.and_then(|r| clamp_capture_region(r, screen_w, screen_h)) {
        return region;
    }
    let mut w = (screen_w.max(2)) & !1;
    let mut h = (screen_h.max(2)) & !1;
    if w < 2 {
        w = 2;
    }
    if h < 2 {
        h = 2;
    }
    CaptureRegion { x: 0, y: 0, w, h }
}

fn start_ffmpeg(
    x_display: &str,
    screen_w: u32,
    screen_h: u32,
    capture_region: Option<CaptureRegion>,
    fps: u32,
    bitrate_override: Option<&str>,
    maxrate_override: Option<&str>,
) -> Option<Child> {
    let capture = effective_capture_region(screen_w, screen_h, capture_region);
    let size = format!("{}x{}", capture.w, capture.h);
    let input_spec = if capture.x == 0 && capture.y == 0 {
        x_display.to_string()
    } else {
        format!("{x_display}+{},{}", capture.x, capture.y)
    };
    let fps_str = fps.to_string();
    // Default GOP: keyframe every ~250ms for fast artifact recovery.
    // BPANE_H264_GOP overrides the absolute value; BPANE_H264_GOP_MULT
    // multiplies fps (legacy, kept for compat).
    let gop = if let Ok(g) = std::env::var("BPANE_H264_GOP") {
        g
    } else {
        let gop_mult = env_or_parse("BPANE_H264_GOP_MULT", 2u32).max(1);
        // If someone explicitly sets GOP_MULT, respect it. Otherwise default
        // to ~250ms keyframe interval (fps/4, minimum 8).
        if std::env::var("BPANE_H264_GOP_MULT").is_ok() {
            (fps.saturating_mul(gop_mult)).to_string()
        } else {
            (fps / 4).max(8).to_string()
        }
    };
    let bitrate = bitrate_override
        .map(String::from)
        .unwrap_or_else(|| env_or_default("BPANE_H264_BITRATE", "2M"));
    let maxrate = maxrate_override
        .map(String::from)
        .unwrap_or_else(|| env_or_default("BPANE_H264_MAXRATE", "4M"));
    let bufsize = env_or_default("BPANE_H264_BUFSIZE", "1M");
    let preset = env_or_default("BPANE_H264_PRESET", "ultrafast");
    let profile = env_or_default("BPANE_H264_PROFILE", "baseline");
    let level = env_or_default("BPANE_H264_LEVEL", "4.2");
    let tune = env_or_default("BPANE_H264_TUNE", "zerolatency");
    let bframes = env_or_default("BPANE_H264_BFRAMES", "0");
    let crf = env::var("BPANE_H264_CRF").ok();

    tracing::info!(
        x_display,
        size = %size,
        fps,
        region_x = capture.x,
        region_y = capture.y,
        "starting ffmpeg"
    );

    let mut cmd = Command::new("ffmpeg");
    cmd.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-draw_mouse",
        "0",
        // Minimize capture/decoder latency
        "-fflags",
        "nobuffer",
        "-flags",
        "low_delay",
        "-probesize",
        "32",
        "-analyzeduration",
        "0",
        "-use_wallclock_as_timestamps",
        "1",
        "-thread_queue_size",
        "4",
        "-f",
        "x11grab",
        "-framerate",
        &fps_str,
        "-video_size",
        &size,
        "-i",
        &input_spec,
        // Convert to yuv420p (4:2:0) — required for H.264 Baseline
        "-pix_fmt",
        "yuv420p",
        "-c:v",
        "libx264",
        "-profile:v",
        &profile,
        "-level",
        &level,
        "-preset",
        &preset,
        // Disable slice-based threading — WebCodecs expects one slice per access unit.
        // `tune=zerolatency` turns on sliced-threads by default, which caused the
        // client to receive multiple slice NALs per frame and fail decoding when
        // each NAL was fed as a separate EncodedVideoChunk. For our low-latency
        // pipeline, keep zerolatency but override sliced-threads to force a single
        // slice per frame.
        "-tune",
        &tune,
        "-x264-params",
        "sliced-threads=0:repeat-headers=1",
        "-g",
        &gop,
        "-bf",
        &bframes,
    ])
    .env("DISPLAY", x_display)
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped());

    if let Some(crf) = crf {
        cmd.args(["-crf", &crf]);
    }

    cmd.args([
        "-b:v", &bitrate, "-maxrate", &maxrate, "-bufsize", &bufsize, "-f", "h264", "-",
    ]);

    match cmd.spawn() {
        Ok(c) => {
            tracing::info!(pid = c.id(), "ffmpeg started");
            Some(c)
        }
        Err(e) => {
            tracing::error!("failed to start ffmpeg: {e}");
            None
        }
    }
}

fn stop_ffmpeg(child: &mut Option<Child>) {
    if let Some(mut c) = child.take() {
        let _ = c.kill();
        let _ = c.wait();
        tracing::debug!("ffmpeg stopped");
    }
}

/// Resize the display using xrandr with the Xorg dummy driver.
/// The dummy driver supports --newmode/--addmode/--output --mode
/// for arbitrary runtime resolution changes. Firefox and other X11
/// clients receive ConfigureNotify events and reflow automatically.
fn resize_display(x_display: &str, w: u32, h: u32) {
    let mode_name = format!("{w}x{h}_60.00");
    tracing::debug!(mode = %mode_name, "xrandr resize");

    // Generate mode timings with cvt
    let cvt = Command::new("cvt")
        .arg(w.to_string())
        .arg(h.to_string())
        .arg("60")
        .output();

    let timings = if let Ok(cvt_out) = cvt {
        let cvt_str = String::from_utf8_lossy(&cvt_out.stdout);
        cvt_str
            .lines()
            .find(|l| l.starts_with("Modeline"))
            .map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                // parts[0] = "Modeline", parts[1] = name, parts[2..] = timings
                parts[2..].join(" ")
            })
    } else {
        None
    };

    if let Some(timings) = timings {
        let timing_args: Vec<&str> = timings.split_whitespace().collect();

        // Add new mode (ignore error if it already exists)
        let _ = Command::new("xrandr")
            .arg("--newmode")
            .arg(&mode_name)
            .args(&timing_args)
            .env("DISPLAY", x_display)
            .output();

        // Add mode to the DUMMY0 output
        let _ = Command::new("xrandr")
            .arg("--addmode")
            .arg("DUMMY0")
            .arg(&mode_name)
            .env("DISPLAY", x_display)
            .output();

        // Switch to the new mode
        let switch = Command::new("xrandr")
            .arg("--output")
            .arg("DUMMY0")
            .arg("--mode")
            .arg(&mode_name)
            .env("DISPLAY", x_display)
            .output();

        match switch {
            Ok(out) if out.status.success() => {
                tracing::debug!(mode = %mode_name, "xrandr mode switch succeeded");
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                tracing::warn!(stderr = %stderr.trim(), "xrandr mode switch failed");
            }
            Err(e) => tracing::error!("xrandr exec failed: {e}"),
        }
    } else {
        tracing::warn!("cvt failed, cannot generate mode timings");
    }

    // Brief pause for the X server to process the mode change
    std::thread::sleep(std::time::Duration::from_millis(100));
}

/// Extract one complete NAL unit from the buffer (Annex B format).
fn extract_nal(buf: &mut Vec<u8>) -> Option<Vec<u8>> {
    if buf.len() < 8 {
        return None;
    }

    let first = find_start_code(buf, 0)?;
    let search_from = first + 3;
    if let Some(second) = find_start_code(buf, search_from) {
        let nal = buf[first..second].to_vec();
        buf.drain(..second);
        return Some(nal);
    }

    if buf.len() > 4 * 1024 * 1024 {
        tracing::warn!(len = buf.len(), "NAL buffer too large, flushing");
        buf.clear();
    }
    None
}

fn env_or_default(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

fn env_or_parse<T: FromStr>(key: &str, default: T) -> T {
    env::var(key)
        .ok()
        .and_then(|v| v.parse::<T>().ok())
        .unwrap_or(default)
}

fn find_start_code(buf: &[u8], from: usize) -> Option<usize> {
    if buf.len() < from + 3 {
        return None;
    }
    for i in from..buf.len() - 2 {
        if buf[i] == 0 && buf[i + 1] == 0 && buf[i + 2] == 1 {
            if i > 0 && buf[i - 1] == 0 {
                return Some(i - 1);
            }
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_nal_from_annex_b() {
        let mut buf = vec![
            0x00, 0x00, 0x00, 0x01, 0x67, 0xAA, 0xBB, 0x00, 0x00, 0x00, 0x01, 0x68, 0xCC, 0xDD,
            0x00, 0x00, 0x00, 0x01, 0x65, 0xEE,
        ];
        let nal1 = extract_nal(&mut buf).unwrap();
        assert_eq!(nal1, vec![0x00, 0x00, 0x00, 0x01, 0x67, 0xAA, 0xBB]);
        let nal2 = extract_nal(&mut buf).unwrap();
        assert_eq!(nal2, vec![0x00, 0x00, 0x00, 0x01, 0x68, 0xCC, 0xDD]);
        assert!(extract_nal(&mut buf).is_none());
    }

    #[test]
    fn extract_nal_3byte_start_code() {
        let mut buf = vec![
            0x00, 0x00, 0x01, 0x41, 0x11, 0x22, 0x00, 0x00, 0x01, 0x41, 0x33,
        ];
        let nal = extract_nal(&mut buf).unwrap();
        assert_eq!(nal, vec![0x00, 0x00, 0x01, 0x41, 0x11, 0x22]);
    }

    #[test]
    fn extract_nal_not_enough_data() {
        let mut buf = vec![0x00, 0x00, 0x01, 0x65, 0xAA];
        assert!(extract_nal(&mut buf).is_none());
    }

    #[test]
    fn clamp_capture_region_clips_and_even_aligns() {
        let region = CaptureRegion {
            x: 127,
            y: 63,
            w: 801,
            h: 601,
        };
        let clipped = clamp_capture_region(region, 1280, 720).expect("region should be valid");
        assert_eq!(clipped.x % 2, 0);
        assert_eq!(clipped.y % 2, 0);
        assert_eq!(clipped.w % 2, 0);
        assert_eq!(clipped.h % 2, 0);
        assert!(clipped.x + clipped.w <= 1280);
        assert!(clipped.y + clipped.h <= 720);
    }

    #[test]
    fn effective_capture_region_defaults_to_full_screen() {
        let full = effective_capture_region(1280, 720, None);
        assert_eq!(
            full,
            CaptureRegion {
                x: 0,
                y: 0,
                w: 1280,
                h: 720
            }
        );
    }

    #[test]
    fn effective_capture_region_uses_valid_tile_region() {
        let region = CaptureRegion {
            x: 128,
            y: 64,
            w: 640,
            h: 384,
        };
        let effective = effective_capture_region(1280, 720, Some(region));
        assert_eq!(effective, region);
    }
}
