use std::sync::Arc;

use bpane_protocol::frame::Frame;
use bpane_protocol::TileMessage;
use tokio::sync::{mpsc, Mutex};
use tracing::{debug, warn};

pub(super) async fn request_full_refresh(
    cached_grid_config: &Mutex<Option<Arc<Frame>>>,
    to_agent: &mpsc::Sender<Frame>,
) -> u64 {
    let gc = cached_grid_config.lock().await;
    let Some(gc_frame) = gc.as_ref() else {
        warn!("no cached GridConfig — cannot request full refresh");
        return 0;
    };

    if gc_frame.payload.len() < 7 {
        warn!("cached GridConfig too short");
        return 0;
    }
    let cols = u16::from_le_bytes([gc_frame.payload[3], gc_frame.payload[4]]);
    let rows = u16::from_le_bytes([gc_frame.payload[5], gc_frame.payload[6]]);
    drop(gc);

    debug!(cols, rows, "requesting full tile refresh for late joiner");

    for row in 0..rows {
        for col in 0..cols {
            let msg = TileMessage::CacheMiss {
                frame_seq: 0,
                col,
                row,
                hash: 0,
            };
            if to_agent.send(msg.to_frame()).await.is_err() {
                warn!("failed to send CacheMiss to agent");
                return u64::from(row) * u64::from(cols) + u64::from(col);
            }
        }
    }
    u64::from(cols) * u64::from(rows)
}
