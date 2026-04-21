use std::sync::Arc;
use std::time::Instant;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;
use tracing::warn;
use wtransport::{Connection, SendStream};

use super::bitrate::DatagramStats;
use super::policy::{adapt_frame_for_client, viewer_can_receive_frame};
use crate::session::Session;
use crate::session_hub::SessionHub;

pub(super) struct EgressTaskContext {
    pub session: Arc<Session>,
    pub hub: Arc<SessionHub>,
    pub session_id: u64,
    pub client_id: u64,
    pub send_stream: Arc<Mutex<SendStream>>,
    pub connection: Connection,
    pub dgram_stats: Arc<DatagramStats>,
}

pub(super) fn spawn_agent_to_browser_task(
    ctx: EgressTaskContext,
    mut from_host: broadcast::Receiver<Arc<Frame>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        while ctx.session.is_active() {
            match from_host.recv().await {
                Ok(frame) => {
                    let is_owner = ctx.hub.is_browser_owner(ctx.client_id);
                    if !is_owner && !viewer_can_receive_frame(&frame) {
                        continue;
                    }

                        if frame.channel == ChannelId::Video {
                            if is_video_keyframe_payload(&frame.payload) {
                                let encoded = frame.encode();
                                let lock_started = Instant::now();
                                let mut stream = ctx.send_stream.lock().await;
                                ctx.hub
                                    .record_egress_send_stream_lock_wait(lock_started.elapsed());
                                if stream.write_all(&encoded).await.is_err() {
                                    break;
                                }
                        } else {
                            match ctx.connection.send_datagram(&frame.payload) {
                                Ok(()) => ctx.dgram_stats.record_success(),
                                Err(_) => ctx.dgram_stats.record_failure(),
                            };
                        }
                        } else {
                            let encoded = adapt_frame_for_client(&frame, is_owner).encode();
                            let lock_started = Instant::now();
                            let mut stream = ctx.send_stream.lock().await;
                            ctx.hub
                                .record_egress_send_stream_lock_wait(lock_started.elapsed());
                            if stream.write_all(&encoded).await.is_err() {
                                break;
                            }
                        }
                    }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    ctx.hub.record_egress_lagged(n as u64);
                    warn!(
                        ctx.session_id,
                        ctx.client_id, n, "client lagged, skipping frames"
                    );
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

fn is_video_keyframe_payload(payload: &[u8]) -> bool {
    payload.len() > 8 && payload[8] != 0
}

#[cfg(test)]
mod tests {
    use super::is_video_keyframe_payload;

    #[test]
    fn video_keyframe_payload_detects_keyframes() {
        let mut payload = vec![0u8; 9];
        payload[8] = 1;
        assert!(is_video_keyframe_payload(&payload));
    }

    #[test]
    fn video_keyframe_payload_rejects_delta_frames() {
        let payload = vec![0u8; 9];
        assert!(!is_video_keyframe_payload(&payload));
    }

    #[test]
    fn video_keyframe_payload_rejects_short_payloads() {
        let payload = vec![0u8; 8];
        assert!(!is_video_keyframe_payload(&payload));
    }
}
