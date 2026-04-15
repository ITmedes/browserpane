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
mod tests;
