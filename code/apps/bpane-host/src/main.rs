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
mod message_dispatch;
mod region;
mod resize;
mod scroll;
mod session;
mod test_session;
mod tile_loop;
pub mod tiles;
mod video_classify;
mod video_region;

use clap::Parser;
use tokio::sync::mpsc;
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use bpane_protocol::frame::Frame;
use bpane_protocol::{ControlMessage, SessionFlags, VideoTileInfo};

use config::preflight_checks;

// ── Small helpers used by session/bridge code ───────────────────────

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

// ── CLI ─────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(name = "bpane-host", about = "BrowserPane host agent daemon")]
struct Args {
    #[arg(long, default_value = "/tmp/bpane.sock")]
    socket: String,
    #[arg(long, default_value_t = 1280)]
    width: u32,
    #[arg(long, default_value_t = 720)]
    height: u32,
    #[arg(long, default_value_t = 30)]
    fps: u32,
}

// ── Entry point ─────────────────────────────────────────────────────

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

    let mut flags =
        SessionFlags::CLIPBOARD | SessionFlags::FILE_TRANSFER | SessionFlags::KEYBOARD_LAYOUT;
    if has_audio {
        flags.insert(SessionFlags::AUDIO | SessionFlags::MICROPHONE);
    }
    if has_camera {
        flags.insert(SessionFlags::CAMERA);
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

// ── Session routing ─────────────────────────────────────────────────

async fn run_session(
    args: &Args,
    display_mode: &display::DisplayMode,
    flags: SessionFlags,
    from_gateway: mpsc::Receiver<Frame>,
    to_gateway: mpsc::Sender<Frame>,
) -> anyhow::Result<()> {
    let ready = ControlMessage::SessionReady { version: 2, flags };
    to_gateway.send(ready.to_frame()).await?;

    let display_str = match display_mode {
        display::DisplayMode::X11 { display } | display::DisplayMode::Xvfb { display } => {
            display.clone()
        }
        _ => String::new(),
    };

    let use_display = !display_str.is_empty() && cfg!(target_os = "linux");
    let has_audio = flags.contains(SessionFlags::AUDIO);

    if use_display {
        info!("using FFmpeg x11grab pipeline");
        session::run_ffmpeg_session(
            args.width,
            args.height,
            args.fps,
            &display_str,
            has_audio,
            from_gateway,
            to_gateway,
        )
        .await
    } else {
        info!("no display available, using test backends");
        test_session::run(args.width, args.height, args.fps, from_gateway, to_gateway).await
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use bpane_protocol::VideoDatagram;

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
}
