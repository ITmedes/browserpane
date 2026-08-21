use std::sync::Arc;

use anyhow::Context;
use bpane_protocol::frame::Frame;
use bpane_protocol::{ClientAccessFlags, ControlMessage};
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::debug;

use super::negotiation::ConnectionProtocol;
use super::policy::{
    adapt_control_message_for_client, adapt_frame_for_client_with_protocol, SessionTransportPolicy,
};

pub(super) struct InitialFramesContext {
    pub joined_as_owner: bool,
    pub initial_access_state: Option<ControlMessage>,
    pub policy: SessionTransportPolicy,
    pub protocol: ConnectionProtocol,
    pub session_id: u64,
    pub client_id: u64,
}

pub(super) async fn send_initial_frames<S>(
    send_stream: &Arc<Mutex<S>>,
    initial_frames: &[Arc<Frame>],
    context: InitialFramesContext,
) -> anyhow::Result<()>
where
    S: AsyncWrite + Unpin + Send + 'static,
{
    let InitialFramesContext {
        joined_as_owner,
        initial_access_state,
        policy,
        protocol,
        session_id,
        client_id,
    } = context;
    let mut stream = send_stream.lock().await;

    for frame in initial_frames {
        if !protocol.allows_server_frame(frame) {
            continue;
        }
        let encoded =
            adapt_frame_for_client_with_protocol(frame, joined_as_owner, &policy, &protocol)
                .encode();
        stream
            .write_all(&encoded)
            .await
            .context("failed to send initial frames")?;
    }

    let had_initial_access_state = initial_access_state.is_some();
    let access_state = initial_access_state.unwrap_or(ControlMessage::ClientAccessState {
        flags: ClientAccessFlags::empty(),
        width: 0,
        height: 0,
    });
    let access_state = adapt_control_message_for_client(access_state, &policy);
    if had_initial_access_state || client_access_state_has_flags(&access_state) {
        let access_frame = access_state.to_frame();
        if protocol.allows_server_frame(&access_frame) {
            stream
                .write_all(&access_frame.encode())
                .await
                .context("failed to send ClientAccessState")?;
        } else if let ControlMessage::ClientAccessState {
            flags,
            width,
            height,
        } = access_state
        {
            if flags.contains(ClientAccessFlags::RESIZE_LOCKED) {
                stream
                    .write_all(
                        &ControlMessage::ResolutionLocked { width, height }
                            .to_frame()
                            .encode(),
                    )
                    .await
                    .context("failed to send ResolutionLocked")?;
            }
        }
        debug!(
            session_id,
            client_id, "sent initial client access state to browser client"
        );
    }

    Ok(())
}

fn client_access_state_has_flags(message: &ControlMessage) -> bool {
    matches!(message, ControlMessage::ClientAccessState { flags, .. } if !flags.is_empty())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bpane_protocol::frame::FrameDecoder;
    use bpane_protocol::{ClientAccessFlags, ControlMessage, SessionFlags};
    use tokio::io::{duplex, AsyncReadExt};
    use tokio::sync::Mutex;

    use crate::session_control::SessionCapabilities;

    use super::super::negotiation::ConnectionProtocol;
    use super::super::policy::{SessionFileTransportPolicy, SessionTransportPolicy};
    use super::{send_initial_frames, InitialFramesContext};

    #[tokio::test]
    async fn non_owner_receives_adapted_ready_and_initial_access_state() {
        let (writer, mut reader) = duplex(4096);
        let send_stream = Arc::new(Mutex::new(writer));
        let initial_frames = vec![Arc::new(
            ControlMessage::SessionReady {
                version: 1,
                flags: SessionFlags::AUDIO
                    | SessionFlags::CLIPBOARD
                    | SessionFlags::FILE_TRANSFER
                    | SessionFlags::MICROPHONE
                    | SessionFlags::CAMERA,
            }
            .to_frame(),
        )];

        send_initial_frames(
            &send_stream,
            &initial_frames,
            InitialFramesContext {
                joined_as_owner: false,
                initial_access_state: Some(ControlMessage::ClientAccessState {
                    flags: ClientAccessFlags::VIEW_ONLY | ClientAccessFlags::RESIZE_LOCKED,
                    width: 1280,
                    height: 720,
                }),
                policy: SessionTransportPolicy::default(),
                protocol: ConnectionProtocol::Legacy,
                session_id: 7,
                client_id: 11,
            },
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 512];
        let n = reader.read(&mut buf).await.unwrap();
        let mut decoder = FrameDecoder::new();
        decoder.push(&buf[..n]).unwrap();

        let ready = decoder.next_frame().unwrap().unwrap();
        let access_state = decoder.next_frame().unwrap().unwrap();

        assert_eq!(ready.payload[0], 0x03);
        assert_ne!(ready.payload[2] & SessionFlags::AUDIO.bits(), 0);
        assert_eq!(ready.payload[2] & SessionFlags::CLIPBOARD.bits(), 0);
        assert_eq!(ready.payload[2] & SessionFlags::FILE_TRANSFER.bits(), 0);
        assert_eq!(ready.payload[2] & SessionFlags::MICROPHONE.bits(), 0);
        assert_eq!(ready.payload[2] & SessionFlags::CAMERA.bits(), 0);

        assert_eq!(
            ControlMessage::decode(&access_state.payload).unwrap(),
            ControlMessage::ClientAccessState {
                flags: ClientAccessFlags::VIEW_ONLY | ClientAccessFlags::RESIZE_LOCKED,
                width: 1280,
                height: 720,
            }
        );
        assert!(decoder.next_frame().unwrap().is_none());
    }

    #[tokio::test]
    async fn resize_locked_collaborator_receives_full_ready_and_lock_only_state() {
        let (writer, mut reader) = duplex(4096);
        let send_stream = Arc::new(Mutex::new(writer));
        let initial_frames = vec![Arc::new(
            ControlMessage::SessionReady {
                version: 1,
                flags: SessionFlags::FILE_TRANSFER | SessionFlags::CAMERA,
            }
            .to_frame(),
        )];

        send_initial_frames(
            &send_stream,
            &initial_frames,
            InitialFramesContext {
                joined_as_owner: true,
                initial_access_state: Some(ControlMessage::ClientAccessState {
                    flags: ClientAccessFlags::RESIZE_LOCKED,
                    width: 1440,
                    height: 900,
                }),
                policy: SessionTransportPolicy::default(),
                protocol: ConnectionProtocol::Legacy,
                session_id: 8,
                client_id: 13,
            },
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 512];
        let n = reader.read(&mut buf).await.unwrap();
        let mut decoder = FrameDecoder::new();
        decoder.push(&buf[..n]).unwrap();

        let ready = decoder.next_frame().unwrap().unwrap();
        let access_state = decoder.next_frame().unwrap().unwrap();
        assert_eq!(
            ControlMessage::decode(&ready.payload).unwrap(),
            ControlMessage::SessionReady {
                version: 1,
                flags: SessionFlags::FILE_TRANSFER | SessionFlags::CAMERA,
            }
        );
        assert_eq!(
            ControlMessage::decode(&access_state.payload).unwrap(),
            ControlMessage::ClientAccessState {
                flags: ClientAccessFlags::RESIZE_LOCKED,
                width: 1440,
                height: 900,
            }
        );
        assert!(decoder.next_frame().unwrap().is_none());
    }

    #[tokio::test]
    async fn owner_receives_initial_frames_without_access_state() {
        let (writer, mut reader) = duplex(4096);
        let send_stream = Arc::new(Mutex::new(writer));
        let initial_frames = vec![Arc::new(
            ControlMessage::SessionReady {
                version: 1,
                flags: SessionFlags::FILE_TRANSFER | SessionFlags::CAMERA,
            }
            .to_frame(),
        )];

        send_initial_frames(
            &send_stream,
            &initial_frames,
            InitialFramesContext {
                joined_as_owner: true,
                initial_access_state: None,
                policy: SessionTransportPolicy::default(),
                protocol: ConnectionProtocol::Legacy,
                session_id: 3,
                client_id: 5,
            },
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 512];
        let n = reader.read(&mut buf).await.unwrap();
        let mut decoder = FrameDecoder::new();
        decoder.push(&buf[..n]).unwrap();

        let ready = decoder.next_frame().unwrap().unwrap();
        assert_eq!(
            ControlMessage::decode(&ready.payload).unwrap(),
            ControlMessage::SessionReady {
                version: 1,
                flags: SessionFlags::FILE_TRANSFER | SessionFlags::CAMERA,
            }
        );
        assert!(decoder.next_frame().unwrap().is_none());
    }

    #[tokio::test]
    async fn owner_receives_access_state_when_session_capabilities_disable_input_or_resize() {
        let (writer, mut reader) = duplex(4096);
        let send_stream = Arc::new(Mutex::new(writer));
        let initial_frames = vec![Arc::new(
            ControlMessage::SessionReady {
                version: 1,
                flags: SessionFlags::FILE_TRANSFER | SessionFlags::KEYBOARD_LAYOUT,
            }
            .to_frame(),
        )];
        let policy = SessionTransportPolicy::from_project_policy_and_capabilities(
            None,
            SessionCapabilities {
                browser_input: false,
                clipboard: true,
                audio: true,
                microphone: true,
                camera: true,
                file_transfer: true,
                resize: false,
            },
        );

        send_initial_frames(
            &send_stream,
            &initial_frames,
            InitialFramesContext {
                joined_as_owner: true,
                initial_access_state: None,
                policy,
                protocol: ConnectionProtocol::Legacy,
                session_id: 4,
                client_id: 6,
            },
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 512];
        let n = reader.read(&mut buf).await.unwrap();
        let mut decoder = FrameDecoder::new();
        decoder.push(&buf[..n]).unwrap();

        let ready = decoder.next_frame().unwrap().unwrap();
        let access_state = decoder.next_frame().unwrap().unwrap();
        assert_eq!(ready.payload[0], 0x03);
        assert_eq!(ready.payload[2] & SessionFlags::KEYBOARD_LAYOUT.bits(), 0);
        assert_eq!(
            ControlMessage::decode(&access_state.payload).unwrap(),
            ControlMessage::ClientAccessState {
                flags: ClientAccessFlags::VIEW_ONLY | ClientAccessFlags::RESIZE_LOCKED,
                width: 0,
                height: 0,
            }
        );
        assert!(decoder.next_frame().unwrap().is_none());
    }

    #[tokio::test]
    async fn owner_session_ready_clears_file_transfer_when_policy_disables_uploads() {
        let (writer, mut reader) = duplex(4096);
        let send_stream = Arc::new(Mutex::new(writer));
        let initial_frames = vec![Arc::new(
            ControlMessage::SessionReady {
                version: 1,
                flags: SessionFlags::FILE_TRANSFER | SessionFlags::CAMERA,
            }
            .to_frame(),
        )];

        send_initial_frames(
            &send_stream,
            &initial_frames,
            InitialFramesContext {
                joined_as_owner: true,
                initial_access_state: None,
                policy: SessionTransportPolicy::with_file_transfer_policy(
                    SessionFileTransportPolicy {
                        allow_browser_uploads: false,
                        allow_browser_downloads: true,
                    },
                ),
                protocol: ConnectionProtocol::Legacy,
                session_id: 3,
                client_id: 5,
            },
        )
        .await
        .unwrap();

        let mut buf = vec![0u8; 512];
        let n = reader.read(&mut buf).await.unwrap();
        let mut decoder = FrameDecoder::new();
        decoder.push(&buf[..n]).unwrap();

        let ready = decoder.next_frame().unwrap().unwrap();
        assert_eq!(ready.payload[0], 0x03);
        assert_eq!(ready.payload[2] & SessionFlags::FILE_TRANSFER.bits(), 0);
        assert_ne!(ready.payload[2] & SessionFlags::CAMERA.bits(), 0);
        assert!(decoder.next_frame().unwrap().is_none());
    }
}
