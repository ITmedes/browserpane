use std::collections::VecDeque;
use std::sync::Arc;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::{Frame, FrameDecoder, ProtocolFailure};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, error, warn};
use wtransport::RecvStream;

use super::negotiation::{map_decoder_error, ConnectionProtocol};
use super::policy::{client_can_forward_frame, SessionTransportPolicy};
use crate::session_files::{new_active_transfer_map, SessionFileRecorder};
use crate::session_hub::{ResizeResult, SessionHub};

use super::session::Session;

pub(super) struct IngressTaskContext {
    pub session: Arc<Session>,
    pub hub: Arc<SessionHub>,
    pub client_id: u64,
    pub recv_stream: RecvStream,
    pub initial_frames: Vec<Frame>,
    pub to_host: mpsc::Sender<Frame>,
    pub file_recorder: SessionFileRecorder,
    pub transport_policy: SessionTransportPolicy,
    pub protocol: ConnectionProtocol,
}

pub(super) fn spawn_browser_to_agent_task(
    context: IngressTaskContext,
) -> JoinHandle<Option<ProtocolFailure>> {
    tokio::spawn(async move {
        let IngressTaskContext {
            session,
            hub,
            client_id,
            mut recv_stream,
            initial_frames,
            to_host,
            file_recorder,
            transport_policy,
            protocol,
        } = context;
        let mut buf = vec![0u8; 64 * 1024];
        let mut decoder = FrameDecoder::new();
        let mut ready_frames = VecDeque::from(initial_frames);
        let mut active_file_transfers = new_active_transfer_map();

        loop {
            if !session.is_active() {
                return None;
            }

            if ready_frames.is_empty() {
                match recv_stream.read(&mut buf).await {
                    Ok(Some(n)) => {
                        session.update_heartbeat().await;

                        if let Err(e) = decoder.push(&buf[..n]) {
                            error!("frame decode error from browser: {e}");
                            return Some(map_decoder_error(e));
                        }

                        loop {
                            match decoder.next_frame() {
                                Ok(Some(frame)) => ready_frames.push_back(frame),
                                Ok(None) => break,
                                Err(e) => {
                                    error!("frame decode error from browser: {e}");
                                    return Some(map_decoder_error(e));
                                }
                            }
                        }
                    }
                    Ok(None) => return None,
                    Err(e) => {
                        warn!("browser read error: {e}");
                        return None;
                    }
                }
            }

            let Some(frame) = ready_frames.pop_front() else {
                continue;
            };
            if let Err(failure) = protocol.validate_client_frame(&frame) {
                return Some(failure);
            }
            let is_owner = hub.is_browser_owner(client_id);

            if let Some((req_w, req_h)) = resolution_request(&frame) {
                if !transport_policy.capabilities.resize {
                    continue;
                }
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

            if !client_can_forward_frame(&frame, is_owner, &transport_policy) {
                continue;
            }

            let forwarded = frame.clone();
            if to_host.send(frame).await.is_err() {
                return None;
            }
            if let Err(error) = file_recorder
                .observe_frame(&mut active_file_transfers, &forwarded)
                .await
            {
                warn!("session file upload metadata recording failed: {error}");
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
