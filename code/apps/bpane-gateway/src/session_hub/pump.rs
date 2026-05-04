use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::Frame;
use tokio::sync::{broadcast, mpsc, Mutex};
use tracing::{info, warn};

use crate::session_files::{new_active_transfer_map, SessionFileRecorder};

pub(super) struct PumpState {
    pub(super) active: Arc<AtomicBool>,
    pub(super) cached_grid_config: Arc<Mutex<Option<Arc<Frame>>>>,
    pub(super) cached_keyframe: Arc<Mutex<Option<Arc<Frame>>>>,
    pub(super) cached_session_ready: Arc<Mutex<Option<Arc<Frame>>>>,
    pub(super) current_resolution: Arc<Mutex<(u16, u16)>>,
}

pub(super) fn spawn(
    mut from_agent: mpsc::Receiver<Frame>,
    broadcast_tx: broadcast::Sender<Arc<Frame>>,
    state: PumpState,
    file_recorder: Option<SessionFileRecorder>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut active_file_transfers = new_active_transfer_map();
        while let Some(frame) = from_agent.recv().await {
            if let Some(recorder) = &file_recorder {
                if let Err(error) = recorder
                    .observe_frame(&mut active_file_transfers, &frame)
                    .await
                {
                    warn!("session file download metadata recording failed: {error}");
                }
            }
            cache_frame(&state, &frame).await;

            let arc_frame = Arc::new(frame);
            let _ = broadcast_tx.send(arc_frame);
        }

        state.active.store(false, Ordering::Relaxed);
        info!("session hub pump ended (agent disconnected)");
    })
}

async fn cache_frame(state: &PumpState, frame: &Frame) {
    if frame.channel == ChannelId::Control && !frame.payload.is_empty() {
        match frame.payload[0] {
            0x02 if frame.payload.len() >= 5 => {
                let w = u16::from_le_bytes([frame.payload[1], frame.payload[2]]);
                let h = u16::from_le_bytes([frame.payload[3], frame.payload[4]]);
                *state.current_resolution.lock().await = (w, h);
            }
            0x03 => {
                *state.cached_session_ready.lock().await = Some(Arc::new(frame.clone()));
            }
            _ => {}
        }
    }

    if frame.channel == ChannelId::Tiles && !frame.payload.is_empty() && frame.payload[0] == 0x01 {
        *state.cached_grid_config.lock().await = Some(Arc::new(frame.clone()));
    }

    if frame.channel == ChannelId::Video && frame.payload.len() > 8 && frame.payload[8] != 0 {
        *state.cached_keyframe.lock().await = Some(Arc::new(frame.clone()));
    }
}
