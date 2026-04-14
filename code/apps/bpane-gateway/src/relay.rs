use bpane_protocol::frame::{Frame, FrameDecoder};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::mpsc;
use tracing::{debug, error, warn};

/// Bidirectional relay between a WebTransport session and a host agent Unix socket.
///
/// The relay reads frames from the agent socket and sends them to the browser,
/// and reads frames from the browser and sends them to the agent socket.
pub struct Relay {
    agent_socket_path: String,
}

impl Relay {
    pub fn new(agent_socket_path: String) -> Self {
        Self { agent_socket_path }
    }

    /// Connect to the host agent and return channels for bidirectional communication.
    /// Returns (frames_from_agent_rx, frames_to_agent_tx).
    pub async fn connect(
        &self,
    ) -> anyhow::Result<(
        mpsc::Receiver<Frame>,
        mpsc::Sender<Frame>,
        tokio::task::JoinHandle<()>,
    )> {
        let stream = UnixStream::connect(&self.agent_socket_path).await?;
        let (read_half, write_half) = stream.into_split();

        let (from_agent_tx, from_agent_rx) = mpsc::channel::<Frame>(256);
        let (to_agent_tx, to_agent_rx) = mpsc::channel::<Frame>(256);

        let handle = tokio::spawn(async move {
            let read_task = Self::read_from_agent(read_half, from_agent_tx);
            let write_task = Self::write_to_agent(write_half, to_agent_rx);

            tokio::select! {
                r = read_task => {
                    if let Err(e) = r {
                        warn!("agent read task ended: {e}");
                    }
                }
                r = write_task => {
                    if let Err(e) = r {
                        warn!("agent write task ended: {e}");
                    }
                }
            }
            debug!("relay to agent closed");
        });

        Ok((from_agent_rx, to_agent_tx, handle))
    }

    async fn read_from_agent(
        mut reader: tokio::net::unix::OwnedReadHalf,
        tx: mpsc::Sender<Frame>,
    ) -> anyhow::Result<()> {
        let mut buf = vec![0u8; 64 * 1024];
        let mut decoder = FrameDecoder::new();

        loop {
            let n = reader.read(&mut buf).await?;
            if n == 0 {
                debug!("agent socket closed (EOF)");
                return Ok(());
            }

            decoder.push(&buf[..n])?;

            loop {
                match decoder.next_frame() {
                    Ok(Some(frame)) => {
                        if tx.send(frame).await.is_err() {
                            return Ok(());
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        error!("frame decode error from agent: {e}");
                        return Err(e.into());
                    }
                }
            }
        }
    }

    async fn write_to_agent(
        mut writer: tokio::net::unix::OwnedWriteHalf,
        mut rx: mpsc::Receiver<Frame>,
    ) -> anyhow::Result<()> {
        while let Some(frame) = rx.recv().await {
            let encoded = frame.encode();
            writer.write_all(&encoded).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bpane_protocol::channel::ChannelId;
    use bytes::BufMut;
    use tokio::net::UnixListener;

    #[tokio::test]
    async fn relay_multiple_frames_sequential() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("multi.sock");
        let sock_path_str = sock_path.to_str().unwrap().to_string();

        let listener = UnixListener::bind(&sock_path).unwrap();

        // Mock agent that echoes all frames back unchanged
        let _agent_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 64 * 1024];
            let mut decoder = FrameDecoder::new();
            loop {
                let n = match stream.read(&mut buf).await {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => break,
                };
                decoder.push(&buf[..n]).unwrap();
                loop {
                    match decoder.next_frame() {
                        Ok(Some(frame)) => {
                            stream.write_all(&frame.encode()).await.unwrap();
                        }
                        Ok(None) => break,
                        Err(e) => panic!("decode error: {e}"),
                    }
                }
            }
        });

        let relay = Relay::new(sock_path_str);
        let (mut from_agent, to_agent, _handle) = relay.connect().await.unwrap();

        // Send 10 frames with different channels and payloads
        let channels = [
            ChannelId::Control,
            ChannelId::Input,
            ChannelId::Cursor,
            ChannelId::Clipboard,
            ChannelId::Tiles,
        ];
        for i in 0..10u8 {
            let frame = Frame::new(
                channels[i as usize % channels.len()],
                vec![i; (i as usize + 1) * 10],
            );
            to_agent.send(frame).await.unwrap();
        }

        // Receive all 10 echoed frames
        for i in 0..10u8 {
            let response =
                tokio::time::timeout(std::time::Duration::from_secs(2), from_agent.recv())
                    .await
                    .unwrap()
                    .unwrap();
            assert_eq!(response.channel, channels[i as usize % channels.len()]);
            assert_eq!(response.payload.len(), (i as usize + 1) * 10);
            assert!(response.payload.iter().all(|&b| b == i));
        }
    }

    #[tokio::test]
    async fn relay_handles_agent_disconnect() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("disconnect.sock");
        let sock_path_str = sock_path.to_str().unwrap().to_string();

        let listener = UnixListener::bind(&sock_path).unwrap();

        // Mock agent that accepts and immediately closes
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            drop(stream); // close immediately
        });

        let relay = Relay::new(sock_path_str);
        let (mut from_agent, _to_agent, _handle) = relay.connect().await.unwrap();

        // from_agent should return None when agent disconnects
        let result =
            tokio::time::timeout(std::time::Duration::from_secs(2), from_agent.recv()).await;
        // Should complete (not hang) and return None
        assert!(result.is_ok());
        assert!(result.unwrap().is_none());
    }

    #[tokio::test]
    async fn relay_connect_fails_for_missing_socket() {
        let relay = Relay::new("/tmp/nonexistent_bpane_test_socket_12345.sock".to_string());
        let result = relay.connect().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn relay_round_trip_via_unix_socket() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");
        let sock_path_str = sock_path.to_str().unwrap().to_string();

        let listener = UnixListener::bind(&sock_path).unwrap();

        // Spawn a mock agent that echoes frames back
        let _agent_handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut buf = vec![0u8; 4096];
            let mut decoder = FrameDecoder::new();
            loop {
                let n = stream.read(&mut buf).await.unwrap();
                if n == 0 {
                    break;
                }
                decoder.push(&buf[..n]).unwrap();
                loop {
                    match decoder.next_frame() {
                        Ok(Some(frame)) => {
                            // Echo back with modified payload
                            let mut new_payload = bytes::BytesMut::from(&frame.payload[..]);
                            new_payload.put_u8(0xFF);
                            let response = Frame::new(frame.channel, new_payload.freeze());
                            stream.write_all(&response.encode()).await.unwrap();
                        }
                        Ok(None) => break,
                        Err(e) => panic!("decode error: {e}"),
                    }
                }
            }
        });

        // Connect relay
        let relay = Relay::new(sock_path_str);
        let (mut from_agent, to_agent, _handle) = relay.connect().await.unwrap();

        // Send a frame through the relay
        let test_frame = Frame::new(ChannelId::Control, vec![0x01, 0x02, 0x03]);
        to_agent.send(test_frame.clone()).await.unwrap();

        // Receive the echoed frame
        let response = from_agent.recv().await.unwrap();
        assert_eq!(response.channel, ChannelId::Control);
        assert_eq!(&response.payload[..], &[0x01, 0x02, 0x03, 0xFF]);

        drop(to_agent);
        drop(from_agent);
    }

    #[tokio::test]
    async fn relay_reassembles_agent_frames_split_across_reads() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("split.sock");
        let sock_path_str = sock_path.to_str().unwrap().to_string();

        let listener = UnixListener::bind(&sock_path).unwrap();
        let response = Frame::new(ChannelId::Control, vec![0xAA, 0xBB, 0xCC]).encode();

        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(&response[..2]).await.unwrap();
            stream.flush().await.unwrap();
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            stream.write_all(&response[2..]).await.unwrap();
            stream.flush().await.unwrap();
        });

        let relay = Relay::new(sock_path_str);
        let (mut from_agent, _to_agent, _handle) = relay.connect().await.unwrap();

        let frame = tokio::time::timeout(std::time::Duration::from_secs(2), from_agent.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(frame.channel, ChannelId::Control);
        assert_eq!(&frame.payload[..], &[0xAA, 0xBB, 0xCC]);
    }
}
