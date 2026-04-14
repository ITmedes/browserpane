use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tracing::{debug, error, info, warn};
use wtransport::{Endpoint, Identity, ServerConfig};

/// Maximum number of concurrent WebTransport sessions.
const MAX_CONCURRENT_SESSIONS: u64 = 100;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::{Frame, FrameDecoder};
use bpane_protocol::{ControlMessage, SessionFlags};

use crate::auth::TokenValidator;
use crate::session::Session;
use crate::session_hub::ResizeResult;
use crate::session_registry::SessionRegistry;

pub struct TransportServer {
    bind_addr: SocketAddr,
    identity: Identity,
    agent_socket_path: String,
    token_validator: Arc<TokenValidator>,
    heartbeat_timeout: Duration,
    registry: Arc<SessionRegistry>,
}

impl TransportServer {
    pub fn new(
        bind_addr: SocketAddr,
        identity: Identity,
        agent_socket_path: String,
        token_validator: Arc<TokenValidator>,
        heartbeat_timeout: Duration,
        registry: Arc<SessionRegistry>,
    ) -> Self {
        Self {
            bind_addr,
            identity,
            agent_socket_path,
            token_validator,
            heartbeat_timeout,
            registry,
        }
    }

    pub async fn run(self) -> anyhow::Result<()> {
        let config = ServerConfig::builder()
            .with_bind_address(self.bind_addr)
            .with_identity(self.identity)
            .keep_alive_interval(Some(Duration::from_secs(3)))
            .build();

        let endpoint = Endpoint::server(config)?;
        info!("WebTransport gateway listening on {}", self.bind_addr);

        let mut session_counter: u64 = 0;
        let active_sessions = Arc::new(AtomicU64::new(0));

        loop {
            let incoming = endpoint.accept().await;
            let session_request = match incoming.await {
                Ok(req) => req,
                Err(e) => {
                    warn!("failed to accept incoming connection: {e}");
                    continue;
                }
            };

            // Enforce session limit
            if active_sessions.load(Ordering::Relaxed) >= MAX_CONCURRENT_SESSIONS {
                warn!("max concurrent sessions ({MAX_CONCURRENT_SESSIONS}) reached, rejecting");
                session_request.not_found().await;
                continue;
            }

            // Extract token from the URL path query
            let path = session_request.path().to_string();
            let token = extract_token(&path);

            if let Some(token) = token {
                if let Err(e) = self.token_validator.validate_token(&token) {
                    warn!("token validation failed: {e}");
                    session_request.not_found().await;
                    continue;
                }
            } else {
                warn!("no token in request path: {path}");
                session_request.not_found().await;
                continue;
            }

            let connection = match session_request.accept().await {
                Ok(conn) => conn,
                Err(e) => {
                    warn!("failed to accept WebTransport session: {e}");
                    continue;
                }
            };

            session_counter += 1;
            let session_id = session_counter;
            let agent_path = self.agent_socket_path.clone();
            let heartbeat_timeout = self.heartbeat_timeout;
            let active_sessions_clone = active_sessions.clone();
            let registry = self.registry.clone();
            active_sessions.fetch_add(1, Ordering::Relaxed);

            info!(
                session_id,
                active = active_sessions.load(Ordering::Relaxed),
                "new WebTransport session accepted"
            );

            tokio::spawn(async move {
                if let Err(e) = handle_session(
                    connection,
                    session_id,
                    &agent_path,
                    heartbeat_timeout,
                    registry.clone(),
                )
                .await
                {
                    error!(session_id, "session error: {e}");
                }
                active_sessions_clone.fetch_sub(1, Ordering::Relaxed);
                info!(session_id, "session ended");
            });
        }
    }
}

fn extract_token(path: &str) -> Option<String> {
    // URL format: /session?token=xxx or /?token=xxx
    let query = path.split('?').nth(1)?;
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("token=") {
            return Some(value.to_string());
        }
    }
    None
}

fn adapt_frame_for_client(frame: &Frame, is_owner: bool) -> Frame {
    if is_owner
        || frame.channel != ChannelId::Control
        || frame.payload.len() < 3
        || frame.payload[0] != 0x03
    {
        return frame.clone();
    }

    let mut payload = frame.payload.to_vec();
    payload[2] &= !(SessionFlags::CLIPBOARD
        | SessionFlags::FILE_TRANSFER
        | SessionFlags::MICROPHONE
        | SessionFlags::CAMERA
        | SessionFlags::KEYBOARD_LAYOUT);
    Frame::new(frame.channel, payload)
}

fn viewer_can_receive_frame(frame: &Frame) -> bool {
    !matches!(frame.channel, ChannelId::Clipboard | ChannelId::FileDown)
}

fn viewer_can_forward_frame(frame: &Frame) -> bool {
    match frame.channel {
        ChannelId::Input
        | ChannelId::Clipboard
        | ChannelId::AudioIn
        | ChannelId::VideoIn
        | ChannelId::FileUp => false,
        ChannelId::Control if !frame.payload.is_empty() && frame.payload[0] == 0x06 => false,
        _ => true,
    }
}

async fn handle_session(
    connection: wtransport::Connection,
    session_id: u64,
    agent_socket_path: &str,
    heartbeat_timeout: Duration,
    registry: Arc<SessionRegistry>,
) -> anyhow::Result<()> {
    // Join the shared session hub (or create a new one)
    let (client_handle, hub) = registry.join(agent_socket_path).await?;
    let client_id = client_handle.client_id;
    let joined_as_owner = client_handle.is_owner;
    let locked_resolution = client_handle.locked_resolution;
    let mut from_host = client_handle.from_host;
    let to_host = client_handle.to_host;
    let initial_frames = client_handle.initial_frames;

    debug!(
        session_id,
        client_id,
        is_owner = joined_as_owner,
        "client joined session hub"
    );

    let session = Arc::new(Session::new(session_id, heartbeat_timeout));

    // Start heartbeat monitor
    let session_clone = session.clone();
    tokio::spawn(async move {
        session_clone.run_heartbeat_monitor().await;
    });

    // Open a bidirectional stream for control
    let (send_stream, mut recv_stream) = connection.open_bi().await?.await?;
    let send_stream = Arc::new(tokio::sync::Mutex::new(send_stream));

    // Send initial frames to late-joining clients (cached SessionReady + keyframe)
    {
        let mut stream = send_stream.lock().await;
        for frame in &initial_frames {
            let encoded = adapt_frame_for_client(frame, joined_as_owner).encode();
            if stream.write_all(&encoded).await.is_err() {
                anyhow::bail!("failed to send initial frames");
            }
        }

        // If non-owner, send ResolutionLocked immediately
        if !joined_as_owner {
            if let Some((w, h)) = locked_resolution {
                let locked = ControlMessage::ResolutionLocked {
                    width: w,
                    height: h,
                };
                let encoded = locked.to_frame().encode();
                if stream.write_all(&encoded).await.is_err() {
                    anyhow::bail!("failed to send ResolutionLocked");
                }
                debug!(
                    session_id,
                    client_id, w, h, "sent ResolutionLocked to non-owner client"
                );
            }
        }
    }

    // Relay: hub broadcast -> browser
    let session_a2b = session.clone();
    let send_stream_clone = send_stream.clone();
    let conn_for_dgram = connection.clone();
    let dgram_stats = Arc::new(DatagramStats::new());
    let dgram_stats_relay = dgram_stats.clone();
    let hub_for_agent_frames = hub.clone();
    let agent_to_browser = tokio::spawn(async move {
        while session_a2b.is_active() {
            match from_host.recv().await {
                Ok(frame) => {
                    let is_owner = hub_for_agent_frames.is_browser_owner(client_id);
                    if !is_owner && !viewer_can_receive_frame(&frame) {
                        continue;
                    }
                    if frame.channel == ChannelId::Video {
                        let payload = &frame.payload;

                        // Check if this is a keyframe by inspecting the
                        // VideoDatagram header: byte 8 is the is_keyframe flag.
                        let is_keyframe = payload.len() > 8 && payload[8] != 0;

                        if is_keyframe {
                            // Keyframes go on the reliable stream — they must
                            // arrive for the decoder to initialize / recover.
                            let encoded = frame.encode();
                            let mut stream = send_stream_clone.lock().await;
                            if stream.write_all(&encoded).await.is_err() {
                                break;
                            }
                        } else {
                            // Delta frames go as best-effort datagrams only —
                            // avoids doubling bandwidth and HOL blocking.
                            match conn_for_dgram.send_datagram(payload) {
                                Ok(()) => dgram_stats_relay.record_success(),
                                Err(_) => dgram_stats_relay.record_failure(),
                            };
                        }
                    } else {
                        let encoded = adapt_frame_for_client(&frame, is_owner).encode();
                        let mut stream = send_stream_clone.lock().await;
                        if stream.write_all(&encoded).await.is_err() {
                            break;
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    warn!(session_id, client_id, n, "client lagged, skipping frames");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    });

    // Bitrate hint task: sample datagram stats every 2 s and send a
    // BitrateHint control message to the browser whenever the adapted
    // bitrate diverges from the last-sent value by more than 10%.
    let session_bh = session.clone();
    let dgram_stats_bh = dgram_stats.clone();
    let send_stream_bh = send_stream.clone();
    let bitrate_hint_task = tokio::spawn(async move {
        // Starting bitrate: 2 Mbps (same default as the host encoder).
        let mut current_bps: u32 = 2_000_000;
        let mut last_sent_bps: u32 = 0; // 0 means no hint sent yet
        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if !session_bh.is_active() {
                break;
            }
            let (successes, failures) = dgram_stats_bh.take_counts();
            if failures > 0 {
                debug!(
                    session_id,
                    client_id, successes, failures, "datagram send failures in last sample window"
                );
            }
            let adapted = compute_adapted_bitrate(current_bps, successes, failures);
            current_bps = adapted;

            // Only send a hint when the new value differs by >10% from the
            // last value we actually sent to the browser.
            let should_send = if last_sent_bps == 0 {
                true
            } else {
                let ratio = adapted as f64 / last_sent_bps as f64;
                ratio < 0.9 || ratio > 1.1
            };

            if should_send {
                let hint = ControlMessage::BitrateHint {
                    target_bps: adapted,
                };
                let encoded = hint.to_frame().encode();
                let mut stream = send_stream_bh.lock().await;
                if stream.write_all(&encoded).await.is_err() {
                    break;
                }
                last_sent_bps = adapted;
                tracing::debug!(
                    session_id,
                    client_id,
                    target_bps = adapted,
                    "sent BitrateHint"
                );
            }
        }
    });

    // Relay: browser -> hub (with resize interception for non-owner clients)
    let session_b2a = session.clone();
    let hub_for_resize = hub.clone();
    let send_stream_resize = send_stream.clone();
    let browser_to_agent = tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        let mut decoder = FrameDecoder::new();
        loop {
            if !session_b2a.is_active() {
                break;
            }
            match recv_stream.read(&mut buf).await {
                Ok(Some(n)) => {
                    session_b2a.update_heartbeat().await;

                    if let Err(e) = decoder.push(&buf[..n]) {
                        error!("frame decode error from browser: {e}");
                        break;
                    }
                    loop {
                        match decoder.next_frame() {
                            Ok(Some(frame)) => {
                                let is_owner = hub_for_resize.is_browser_owner(client_id);
                                // Intercept ResolutionRequest from non-owner clients
                                if !is_owner
                                    && frame.channel == ChannelId::Control
                                    && !frame.payload.is_empty()
                                    && frame.payload[0] == 0x01
                                    && frame.payload.len() >= 5
                                {
                                    let req_w =
                                        u16::from_le_bytes([frame.payload[1], frame.payload[2]]);
                                    let req_h =
                                        u16::from_le_bytes([frame.payload[3], frame.payload[4]]);

                                    match hub_for_resize
                                        .request_resize(client_id, req_w, req_h)
                                        .await
                                    {
                                        ResizeResult::Applied => {
                                            // Should not happen for non-owner
                                        }
                                        ResizeResult::Locked(w, h) => {
                                            if w > 0 && h > 0 {
                                                let locked = ControlMessage::ResolutionLocked {
                                                    width: w,
                                                    height: h,
                                                };
                                                let encoded = locked.to_frame().encode();
                                                let mut stream = send_stream_resize.lock().await;
                                                let _ = stream.write_all(&encoded).await;
                                            }
                                        }
                                    }
                                    continue;
                                }

                                // Owner's ResolutionRequest goes through the hub
                                if is_owner
                                    && frame.channel == ChannelId::Control
                                    && !frame.payload.is_empty()
                                    && frame.payload[0] == 0x01
                                    && frame.payload.len() >= 5
                                {
                                    let req_w =
                                        u16::from_le_bytes([frame.payload[1], frame.payload[2]]);
                                    let req_h =
                                        u16::from_le_bytes([frame.payload[3], frame.payload[4]]);
                                    let _ = hub_for_resize
                                        .request_resize(client_id, req_w, req_h)
                                        .await;
                                    continue;
                                }

                                if !is_owner && !viewer_can_forward_frame(&frame) {
                                    continue;
                                }

                                // All other frames: forward to host
                                if to_host.send(frame).await.is_err() {
                                    return;
                                }
                            }
                            Ok(None) => break,
                            Err(e) => {
                                error!("frame decode error from browser: {e}");
                                return;
                            }
                        }
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    warn!("browser read error: {e}");
                    break;
                }
            }
        }
    });

    // Gateway-originated periodic pings toward browser
    let session_ping = session.clone();
    let send_stream_ping = send_stream.clone();
    let gateway_pinger = tokio::spawn(async move {
        let mut seq: u32 = 0;
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            interval.tick().await;
            if !session_ping.is_active() {
                break;
            }
            seq += 1;
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let ping = ControlMessage::Ping {
                seq,
                timestamp_ms: now,
            };
            let encoded = ping.to_frame().encode();
            let mut stream = send_stream_ping.lock().await;
            if stream.write_all(&encoded).await.is_err() {
                break;
            }
        }
    });

    tokio::select! {
        _ = agent_to_browser => {}
        _ = browser_to_agent => {}
        _ = gateway_pinger => {}
        _ = bitrate_hint_task => {}
    }

    session.deactivate();
    registry.leave(agent_socket_path, client_id).await;

    // Explicitly close the QUIC connection so Chrome doesn't try to reuse it
    // for subsequent WebTransport sessions. Without this, Chrome's HTTP/3
    // connection pooling sends new CONNECT requests on the stale connection,
    // which wtransport can't handle (one session per QUIC connection).
    connection.close(wtransport::VarInt::from_u32(0), b"session ended");

    Ok(())
}

/// Tracks datagram send success/failure counts for bitrate adaptation (Phase 9).
struct DatagramStats {
    successes: std::sync::atomic::AtomicU64,
    failures: std::sync::atomic::AtomicU64,
}

impl DatagramStats {
    fn new() -> Self {
        Self {
            successes: std::sync::atomic::AtomicU64::new(0),
            failures: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn record_success(&self) {
        self.successes
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn record_failure(&self) {
        self.failures
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Take and reset the counters. Returns (successes, failures).
    fn take_counts(&self) -> (u64, u64) {
        let s = self.successes.swap(0, std::sync::atomic::Ordering::Relaxed);
        let f = self.failures.swap(0, std::sync::atomic::Ordering::Relaxed);
        (s, f)
    }
}

/// Compute an adapted bitrate given the current bitrate and datagram
/// success/failure counts observed during the last sample window.
///
/// Rules:
/// - >10% failure   → reduce by 20%
/// -  2–10% failure → reduce by 5%
/// -  0% failure    → increase by 5%
/// - otherwise      → no change
///
/// The result is clamped to [200 kbps, 8 Mbps].
fn compute_adapted_bitrate(current_bps: u32, successes: u64, failures: u64) -> u32 {
    let total = successes + failures;
    if total == 0 {
        return current_bps;
    }
    let failure_rate = failures as f32 / total as f32;
    let new = if failure_rate > 0.10 {
        (current_bps as f32 * 0.8) as u32
    } else if failure_rate > 0.02 {
        (current_bps as f32 * 0.95) as u32
    } else if failure_rate < 0.01 && failures == 0 {
        (current_bps as f32 * 1.05) as u32
    } else {
        current_bps
    };
    new.clamp(200_000, 8_000_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_token_from_path() {
        assert_eq!(
            extract_token("/session?token=abc123"),
            Some("abc123".to_string())
        );
        assert_eq!(
            extract_token("/?token=xyz&other=1"),
            Some("xyz".to_string())
        );
        assert_eq!(extract_token("/session"), None);
        assert_eq!(extract_token("/session?other=1"), None);
    }

    // ── DatagramStats tests (Phase 9) ───────────────────────────────

    #[test]
    fn datagram_stats_initial_zero() {
        let stats = DatagramStats::new();
        let (s, f) = stats.take_counts();
        assert_eq!(s, 0);
        assert_eq!(f, 0);
    }

    #[test]
    fn datagram_stats_counts_success_and_failure() {
        let stats = DatagramStats::new();
        stats.record_success();
        stats.record_success();
        stats.record_success();
        stats.record_failure();
        let (s, f) = stats.take_counts();
        assert_eq!(s, 3);
        assert_eq!(f, 1);
    }

    #[test]
    fn datagram_stats_take_resets_counters() {
        let stats = DatagramStats::new();
        stats.record_success();
        stats.record_failure();
        let (s, f) = stats.take_counts();
        assert_eq!(s, 1);
        assert_eq!(f, 1);
        // After take, counters should be zero
        let (s2, f2) = stats.take_counts();
        assert_eq!(s2, 0);
        assert_eq!(f2, 0);
    }

    #[test]
    fn datagram_stats_concurrent_access() {
        use std::sync::Arc;
        let stats = Arc::new(DatagramStats::new());
        let mut handles = Vec::new();
        for _ in 0..10 {
            let s = stats.clone();
            handles.push(std::thread::spawn(move || {
                for _ in 0..100 {
                    s.record_success();
                    s.record_failure();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        let (s, f) = stats.take_counts();
        assert_eq!(s, 1000);
        assert_eq!(f, 1000);
    }

    #[test]
    fn bitrate_adapts_down_on_high_failure() {
        // >10% failure → 20% reduction
        let result = compute_adapted_bitrate(2_000_000, 80, 20);
        assert_eq!(result, 1_600_000);
    }

    #[test]
    fn bitrate_adapts_down_on_moderate_failure() {
        // 2-10% failure → 5% reduction
        let result = compute_adapted_bitrate(2_000_000, 95, 5);
        assert_eq!(result, 1_900_000);
    }

    #[test]
    fn bitrate_adapts_up_on_zero_failure() {
        // 0% failure → 5% increase
        let result = compute_adapted_bitrate(2_000_000, 100, 0);
        assert_eq!(result, 2_100_000);
    }

    #[test]
    fn bitrate_stays_same_on_low_failure() {
        // ~1% failure → no change
        let result = compute_adapted_bitrate(2_000_000, 99, 1);
        assert_eq!(result, 2_000_000);
    }

    #[test]
    fn bitrate_clamps_to_minimum() {
        // Repeated failures should never go below 200kbps
        let mut bps = 300_000u32;
        for _ in 0..10 {
            bps = compute_adapted_bitrate(bps, 5, 50);
        }
        assert!(bps >= 200_000);
    }

    #[test]
    fn bitrate_clamps_to_maximum() {
        // Repeated successes should never exceed 8Mbps
        let mut bps = 7_500_000u32;
        for _ in 0..50 {
            bps = compute_adapted_bitrate(bps, 100, 0);
        }
        assert!(bps <= 8_000_000);
    }

    #[test]
    fn bitrate_no_change_on_zero_traffic() {
        let result = compute_adapted_bitrate(2_000_000, 0, 0);
        assert_eq!(result, 2_000_000);
    }

    #[test]
    fn adapt_frame_for_client_strips_viewer_only_capabilities() {
        let frame = ControlMessage::SessionReady {
            version: 1,
            flags: SessionFlags::new(
                SessionFlags::AUDIO
                    | SessionFlags::CLIPBOARD
                    | SessionFlags::FILE_TRANSFER
                    | SessionFlags::MICROPHONE
                    | SessionFlags::CAMERA
                    | SessionFlags::KEYBOARD_LAYOUT,
            ),
        }
        .to_frame();

        let adapted = adapt_frame_for_client(&frame, false);

        assert_eq!(adapted.payload[0], 0x03);
        assert_ne!(adapted.payload[2] & SessionFlags::AUDIO, 0);
        assert_eq!(adapted.payload[2] & SessionFlags::CLIPBOARD, 0);
        assert_eq!(adapted.payload[2] & SessionFlags::FILE_TRANSFER, 0);
        assert_eq!(adapted.payload[2] & SessionFlags::MICROPHONE, 0);
        assert_eq!(adapted.payload[2] & SessionFlags::CAMERA, 0);
        assert_eq!(adapted.payload[2] & SessionFlags::KEYBOARD_LAYOUT, 0);
    }

    #[test]
    fn adapt_frame_for_client_leaves_owner_flags_unchanged() {
        let frame = ControlMessage::SessionReady {
            version: 1,
            flags: SessionFlags::new(SessionFlags::FILE_TRANSFER | SessionFlags::CAMERA),
        }
        .to_frame();

        let adapted = adapt_frame_for_client(&frame, true);
        assert_eq!(adapted, frame);
    }

    #[test]
    fn viewer_can_receive_frame_blocks_clipboard_and_download() {
        let clipboard = Frame::new(ChannelId::Clipboard, vec![0x01]);
        let download = Frame::new(ChannelId::FileDown, vec![0x01]);
        let video = Frame::new(ChannelId::Video, vec![0x00]);

        assert!(!viewer_can_receive_frame(&clipboard));
        assert!(!viewer_can_receive_frame(&download));
        assert!(viewer_can_receive_frame(&video));
    }

    #[test]
    fn viewer_can_forward_frame_blocks_interactive_channels() {
        let input = Frame::new(ChannelId::Input, vec![0x01]);
        let clipboard = Frame::new(ChannelId::Clipboard, vec![0x01]);
        let audio_in = Frame::new(ChannelId::AudioIn, vec![0x01]);
        let video_in = Frame::new(ChannelId::VideoIn, vec![0x01]);
        let file_up = Frame::new(ChannelId::FileUp, vec![0x01]);
        let layout = Frame::new(ChannelId::Control, vec![0x06, 0x00]);
        let pong = Frame::new(ChannelId::Control, vec![0x05]);

        assert!(!viewer_can_forward_frame(&input));
        assert!(!viewer_can_forward_frame(&clipboard));
        assert!(!viewer_can_forward_frame(&audio_in));
        assert!(!viewer_can_forward_frame(&video_in));
        assert!(!viewer_can_forward_frame(&file_up));
        assert!(!viewer_can_forward_frame(&layout));
        assert!(viewer_can_forward_frame(&pong));
    }
}
