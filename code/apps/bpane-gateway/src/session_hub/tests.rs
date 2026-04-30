use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::FrameDecoder;
use bpane_protocol::{ClientAccessFlags, ControlMessage, SessionFlags, TileMessage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

use super::{ResizeResult, SessionHub, SubscribeError};

mod lifecycle;
mod mcp;
mod ownership;
mod telemetry;
mod termination;

async fn mock_agent(sock_path: &str) -> tokio::task::JoinHandle<()> {
    let listener = UnixListener::bind(sock_path).unwrap();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut buf = vec![0u8; 64 * 1024];
        let mut decoder = FrameDecoder::new();

        let ready = ControlMessage::SessionReady {
            version: 1,
            flags: SessionFlags::KEYBOARD_LAYOUT,
        };
        stream.write_all(&ready.to_frame().encode()).await.unwrap();

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
                        if frame.channel == ChannelId::Control
                            && !frame.payload.is_empty()
                            && frame.payload[0] == 0x01
                            && frame.payload.len() >= 5
                        {
                            let ack = ControlMessage::ResolutionAck {
                                width: u16::from_le_bytes([frame.payload[1], frame.payload[2]]),
                                height: u16::from_le_bytes([frame.payload[3], frame.payload[4]]),
                            };
                            stream.write_all(&ack.to_frame().encode()).await.unwrap();
                        } else if frame.channel == ChannelId::Tiles {
                            if let Ok(TileMessage::CacheMiss { col, row, .. }) =
                                TileMessage::decode(&frame.payload)
                            {
                                let fill = TileMessage::Fill {
                                    col,
                                    row,
                                    rgba: 0xFF00_0000,
                                };
                                stream.write_all(&fill.to_frame().encode()).await.unwrap();
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => panic!("decode error: {e}"),
                }
            }
        }
    })
}

async fn expect_control_message_eventually(
    rx: &mut tokio::sync::mpsc::Receiver<ControlMessage>,
    expected: ControlMessage,
) {
    for _ in 0..4 {
        let message = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .unwrap();
        if message == Some(expected.clone()) {
            return;
        }
    }

    panic!("did not receive expected control message: {expected:?}");
}
