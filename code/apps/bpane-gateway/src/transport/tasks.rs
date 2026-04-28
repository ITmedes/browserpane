use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bpane_protocol::ControlMessage;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::debug;

use super::bitrate::{compute_adapted_bitrate, DatagramStats};
use crate::session::Session;
use crate::session_hub::BrowserClientRole;

pub(super) fn recorder_role_suppresses_bitrate_feedback(client_role: BrowserClientRole) -> bool {
    !client_role.allows_bitrate_feedback()
}

pub(super) fn spawn_bitrate_hint_task<S>(
    session_id: u64,
    client_id: u64,
    session: Arc<Session>,
    dgram_stats: Arc<DatagramStats>,
    send_stream: Arc<Mutex<S>>,
) -> JoinHandle<()>
where
    S: AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut current_bps: u32 = 2_000_000;
        let mut last_sent_bps: u32 = 0;

        loop {
            tokio::time::sleep(Duration::from_secs(2)).await;
            if !session.is_active() {
                break;
            }

            let (successes, failures) = dgram_stats.take_counts();
            if failures > 0 {
                debug!(
                    session_id,
                    client_id, successes, failures, "datagram send failures in last sample window"
                );
            }

            let adapted = compute_adapted_bitrate(current_bps, successes, failures);
            current_bps = adapted;

            let should_send = if last_sent_bps == 0 {
                true
            } else {
                let ratio = adapted as f64 / last_sent_bps as f64;
                !(0.9..=1.1).contains(&ratio)
            };

            if should_send {
                let hint = ControlMessage::BitrateHint {
                    target_bps: adapted,
                };
                let encoded = hint.to_frame().encode();
                let mut stream = send_stream.lock().await;
                if stream.write_all(&encoded).await.is_err() {
                    break;
                }
                last_sent_bps = adapted;
                debug!(
                    session_id,
                    client_id,
                    target_bps = adapted,
                    "sent BitrateHint"
                );
            }
        }
    })
}

pub(super) fn spawn_gateway_pinger<S>(
    session: Arc<Session>,
    send_stream: Arc<Mutex<S>>,
) -> JoinHandle<()>
where
    S: AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut seq: u32 = 0;
        let mut interval = tokio::time::interval(Duration::from_secs(5));

        loop {
            interval.tick().await;
            if !session.is_active() {
                break;
            }

            seq += 1;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64;
            let ping = ControlMessage::Ping {
                seq,
                timestamp_ms: now,
            };
            let encoded = ping.to_frame().encode();
            let mut stream = send_stream.lock().await;
            if stream.write_all(&encoded).await.is_err() {
                break;
            }
        }
    })
}

#[cfg(test)]
mod tests;

pub(super) fn spawn_direct_control_task<S>(
    session: Arc<Session>,
    send_stream: Arc<Mutex<S>>,
    mut control_rx: tokio::sync::mpsc::Receiver<ControlMessage>,
) -> JoinHandle<()>
where
    S: AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        while let Some(message) = control_rx.recv().await {
            if !session.is_active() {
                break;
            }

            let encoded = message.to_frame().encode();
            let mut stream = send_stream.lock().await;
            if stream.write_all(&encoded).await.is_err() {
                break;
            }
        }
    })
}
