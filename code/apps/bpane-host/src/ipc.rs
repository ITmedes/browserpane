use bpane_protocol::frame::{Frame, FRAME_HEADER_SIZE};
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

/// IPC server that listens on a Unix domain socket for gateway connections.
/// Frames are length-prefixed binary as per the protocol spec.
pub struct IpcServer {
    listener: UnixListener,
}

impl IpcServer {
    pub fn bind(path: &str) -> anyhow::Result<Self> {
        // Remove stale socket file if it exists
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        info!("IPC listening on {path}");
        Ok(Self { listener })
    }

    /// Accept a single connection and return channels for communication.
    pub async fn accept(&self) -> anyhow::Result<(mpsc::Receiver<Frame>, mpsc::Sender<Frame>)> {
        let (stream, _) = self.listener.accept().await?;
        info!("gateway connected via IPC");

        let (read_half, write_half) = stream.into_split();
        let (from_gateway_tx, from_gateway_rx) = mpsc::channel::<Frame>(256);
        let (to_gateway_tx, to_gateway_rx) = mpsc::channel::<Frame>(256);

        // Read task: gateway -> host
        tokio::spawn(async move {
            let mut reader = read_half;
            let mut buf = vec![0u8; 64 * 1024];
            let mut pending = BytesMut::new();
            const MAX_PENDING: usize = 4 * 1024 * 1024; // 4 MiB

            loop {
                let n = match reader.read(&mut buf).await {
                    Ok(0) => {
                        debug!("IPC connection closed");
                        break;
                    }
                    Ok(n) => n,
                    Err(e) => {
                        warn!("IPC read error: {e}");
                        break;
                    }
                };

                pending.extend_from_slice(&buf[..n]);
                if pending.len() > MAX_PENDING {
                    error!(
                        "IPC pending buffer exceeds {} bytes, disconnecting",
                        MAX_PENDING
                    );
                    break;
                }

                loop {
                    if pending.len() < FRAME_HEADER_SIZE {
                        break;
                    }
                    let declared_len =
                        u32::from_le_bytes([pending[1], pending[2], pending[3], pending[4]])
                            as usize;
                    let total_size = FRAME_HEADER_SIZE + declared_len;
                    if pending.len() < total_size {
                        break;
                    }
                    // Zero-copy: split off the frame bytes, freeze, then decode
                    let frame_bytes = pending.split_to(total_size).freeze();
                    match Frame::decode_bytes(frame_bytes) {
                        Ok((frame, _consumed)) => {
                            if from_gateway_tx.send(frame).await.is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            error!("IPC frame decode error: {e}");
                            return;
                        }
                    }
                }
            }
        });

        // Write task: host -> gateway
        tokio::spawn(async move {
            let mut writer = write_half;
            let mut rx = to_gateway_rx;
            while let Some(frame) = rx.recv().await {
                let encoded = frame.encode();
                if let Err(e) = writer.write_all(&encoded).await {
                    warn!("IPC write error: {e}");
                    break;
                }
            }
        });

        Ok((from_gateway_rx, to_gateway_tx))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bpane_protocol::channel::ChannelId;
    use tokio::net::UnixStream;

    #[tokio::test]
    async fn ipc_multiple_frames() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("multi.sock");
        let sock_path_str = sock_path.to_str().unwrap();

        let server = IpcServer::bind(sock_path_str).unwrap();

        let path = sock_path_str.to_string();
        let connect_task = tokio::spawn(async move {
            let mut stream = UnixStream::connect(&path).await.unwrap();

            // Send 5 frames on different channels
            let channels = [
                ChannelId::Control,
                ChannelId::Input,
                ChannelId::Cursor,
                ChannelId::Clipboard,
                ChannelId::Tiles,
            ];
            for (i, ch) in channels.iter().enumerate() {
                let frame = Frame::new(*ch, vec![i as u8; i + 1]);
                stream.write_all(&frame.encode()).await.unwrap();
            }
            stream.flush().await.unwrap();
            channels.len()
        });

        let (mut from_gateway, _to_gateway) = server.accept().await.unwrap();

        let expected_count = connect_task.await.unwrap();
        let channels = [
            ChannelId::Control,
            ChannelId::Input,
            ChannelId::Cursor,
            ChannelId::Clipboard,
            ChannelId::Tiles,
        ];

        for i in 0..expected_count {
            let frame =
                tokio::time::timeout(std::time::Duration::from_secs(2), from_gateway.recv())
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(frame.channel, channels[i]);
            assert_eq!(frame.payload.len(), i + 1);
            assert!(frame.payload.iter().all(|&b| b == i as u8));
        }
    }

    #[tokio::test]
    async fn ipc_handles_client_disconnect() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("disconnect.sock");
        let sock_path_str = sock_path.to_str().unwrap();

        let server = IpcServer::bind(sock_path_str).unwrap();

        let path = sock_path_str.to_string();
        tokio::spawn(async move {
            let mut stream = UnixStream::connect(&path).await.unwrap();
            let frame = Frame::new(ChannelId::Control, vec![0x01]);
            stream.write_all(&frame.encode()).await.unwrap();
            // Drop stream (disconnect)
        });

        let (mut from_gateway, _to_gateway) = server.accept().await.unwrap();

        // Should get the one frame
        let frame = tokio::time::timeout(std::time::Duration::from_secs(2), from_gateway.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(frame.channel, ChannelId::Control);

        // Then should get None (channel closed)
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(2), from_gateway.recv()).await;
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn ipc_removes_stale_socket() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("stale.sock");
        let sock_path_str = sock_path.to_str().unwrap();

        // Create a stale socket file
        std::fs::write(&sock_path, b"stale").unwrap();
        assert!(sock_path.exists());

        // Should succeed by removing the stale file
        let _server = IpcServer::bind(sock_path_str).unwrap();
    }

    #[tokio::test]
    async fn ipc_frame_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let sock_path_str = sock_path.to_str().unwrap();

        let server = IpcServer::bind(sock_path_str).unwrap();

        // Connect as a mock gateway
        let connect_task = tokio::spawn({
            let path = sock_path_str.to_string();
            async move {
                let mut stream = UnixStream::connect(&path).await.unwrap();
                // Send a frame
                let frame = Frame::new(ChannelId::Control, vec![0x01, 0x02]);
                stream.write_all(&frame.encode()).await.unwrap();

                // Read back the response
                let mut buf = vec![0u8; 1024];
                let n = stream.read(&mut buf).await.unwrap();
                let (response, _) = Frame::decode(&buf[..n]).unwrap();
                response
            }
        });

        // Accept connection
        let (mut from_gateway, to_gateway) = server.accept().await.unwrap();

        // Read the incoming frame
        let received = from_gateway.recv().await.unwrap();
        assert_eq!(received.channel, ChannelId::Control);
        assert_eq!(&received.payload[..], &[0x01, 0x02]);

        // Send a response
        let response = Frame::new(ChannelId::Control, vec![0x03, 0x04]);
        to_gateway.send(response).await.unwrap();

        let client_response = connect_task.await.unwrap();
        assert_eq!(client_response.channel, ChannelId::Control);
        assert_eq!(&client_response.payload[..], &[0x03, 0x04]);
    }
}
