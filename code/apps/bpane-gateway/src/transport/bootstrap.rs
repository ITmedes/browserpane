use std::sync::Arc;

use anyhow::Context;
use bpane_protocol::frame::Frame;
use bpane_protocol::ControlMessage;
use tokio::io::{AsyncWrite, AsyncWriteExt};
use tokio::sync::Mutex;
use tracing::debug;

use super::policy::adapt_frame_for_client;

pub(super) async fn send_initial_frames<S>(
    send_stream: &Arc<Mutex<S>>,
    initial_frames: &[Arc<Frame>],
    joined_as_owner: bool,
    initial_access_state: Option<ControlMessage>,
    session_id: u64,
    client_id: u64,
) -> anyhow::Result<()>
where
    S: AsyncWrite + Unpin + Send + 'static,
{
    let mut stream = send_stream.lock().await;

    for frame in initial_frames {
        let encoded = adapt_frame_for_client(frame, joined_as_owner).encode();
        stream
            .write_all(&encoded)
            .await
            .context("failed to send initial frames")?;
    }

    if let Some(access_state) = initial_access_state {
        let encoded = access_state.to_frame().encode();
        stream
            .write_all(&encoded)
            .await
            .context("failed to send ClientAccessState")?;
        debug!(
            session_id,
            client_id, "sent initial client access state to browser client"
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use bpane_protocol::frame::FrameDecoder;
    use bpane_protocol::{ClientAccessFlags, ControlMessage, SessionFlags};
    use tokio::io::{duplex, AsyncReadExt};
    use tokio::sync::Mutex;

    use super::send_initial_frames;

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
            false,
            Some(ControlMessage::ClientAccessState {
                flags: ClientAccessFlags::VIEW_ONLY | ClientAccessFlags::RESIZE_LOCKED,
                width: 1280,
                height: 720,
            }),
            7,
            11,
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
            true,
            Some(ControlMessage::ClientAccessState {
                flags: ClientAccessFlags::RESIZE_LOCKED,
                width: 1440,
                height: 900,
            }),
            8,
            13,
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

        send_initial_frames(&send_stream, &initial_frames, true, None, 3, 5)
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
}
