use std::sync::Arc;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::{Frame, FrameDecoder};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};
use wtransport::RecvStream;

use super::policy::viewer_can_forward_frame;
use crate::session_hub::{ResizeResult, SessionHub};

use super::session::Session;

pub(super) fn spawn_browser_to_agent_task(
    session: Arc<Session>,
    hub: Arc<SessionHub>,
    client_id: u64,
    mut recv_stream: RecvStream,
    to_host: mpsc::Sender<Frame>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut buf = vec![0u8; 64 * 1024];
        let mut decoder = FrameDecoder::new();

        loop {
            if !session.is_active() {
                break;
            }

            match recv_stream.read(&mut buf).await {
                Ok(Some(n)) => {
                    session.update_heartbeat().await;

                    if let Err(e) = decoder.push(&buf[..n]) {
                        error!("frame decode error from browser: {e}");
                        break;
                    }

                    loop {
                        match decoder.next_frame() {
                            Ok(Some(frame)) => {
                                let is_owner = hub.is_browser_owner(client_id);

                                if let Some((req_w, req_h)) = resolution_request(&frame) {
                                    match hub.request_resize(client_id, req_w, req_h).await {
                                        ResizeResult::Applied => {}
                                        ResizeResult::Locked(width, height) => {
                                            debug!(
                                                client_id,
                                                requested_width = req_w,
                                                requested_height = req_h,
                                                locked_width = width,
                                                locked_height = height,
                                                "ignored resize request because the session resolution is locked"
                                            );
                                        }
                                    }
                                    continue;
                                }

                                if !is_owner && !viewer_can_forward_frame(&frame) {
                                    continue;
                                }

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
    })
}

fn resolution_request(frame: &Frame) -> Option<(u16, u16)> {
    if frame.channel != ChannelId::Control || frame.payload.len() < 5 || frame.payload[0] != 0x01 {
        return None;
    }

    Some((
        u16::from_le_bytes([frame.payload[1], frame.payload[2]]),
        u16::from_le_bytes([frame.payload[3], frame.payload[4]]),
    ))
}

#[cfg(test)]
mod tests {
    use bpane_protocol::channel::ChannelId;
    use bpane_protocol::frame::Frame;
    use bpane_protocol::ControlMessage;

    use super::resolution_request;

    #[test]
    fn resolution_request_extracts_dimensions() {
        let frame = ControlMessage::ResolutionRequest {
            width: 1280,
            height: 720,
        }
        .to_frame();

        assert_eq!(resolution_request(&frame), Some((1280, 720)));
    }

    #[test]
    fn resolution_request_ignores_other_control_messages() {
        let frame = ControlMessage::Ping {
            seq: 1,
            timestamp_ms: 2,
        }
        .to_frame();

        assert_eq!(resolution_request(&frame), None);
    }

    #[test]
    fn resolution_request_ignores_non_control_frames() {
        let frame = Frame::new(ChannelId::Input, vec![0x01, 0x00, 0x05, 0x00, 0x07]);

        assert_eq!(resolution_request(&frame), None);
    }
}
