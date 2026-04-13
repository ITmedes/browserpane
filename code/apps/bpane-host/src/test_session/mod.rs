//! Fallback test session using test backends (non-Linux or no display).

#[cfg(test)]
mod tests;

use std::time::Duration;

use tokio::sync::mpsc;
use tracing::info;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::{Frame, Message};
use bpane_protocol::{ControlMessage, VideoDatagram};

use crate::capture::{CaptureBackend, TestCaptureBackend};
use crate::encode::{EncodeBackend, TestEncoder};
use crate::input::{InputBackend, TestInputBackend};
use crate::resize;

/// Run a test session with synthetic capture + encode backends.
pub async fn run(
    width: u32,
    height: u32,
    fps: u32,
    mut from_gateway: mpsc::Receiver<Frame>,
    to_gateway: mpsc::Sender<Frame>,
) -> anyhow::Result<()> {
    let mut capture = TestCaptureBackend::new(width, height);
    let mut encoder = TestEncoder::new(width, height);
    let mut input_backend: Box<dyn InputBackend> = Box::new(TestInputBackend::default());
    let mut resize_handler = resize::ResizeHandler::new(width, height);
    let mut nal_id: u32 = 0;
    let frame_interval = Duration::from_micros(1_000_000 / fps as u64);

    let (video_tx, mut video_rx) = mpsc::channel::<Frame>(64);
    let video_gateway = to_gateway.clone();
    let video_task = tokio::spawn(async move {
        while let Some(frame) = video_rx.recv().await {
            if video_gateway.send(frame).await.is_err() {
                break;
            }
        }
    });

    let mut frame_timer = tokio::time::interval(frame_interval);
    let mut ping_seq: u32 = 0;
    let mut ping_timer = tokio::time::interval(Duration::from_secs(5));

    let max_frag = super::video_datagram_max_fragment_size(None);

    let result = loop {
        tokio::select! {
            _ = frame_timer.tick() => {
                if let Ok(Some(raw_frame)) = capture.capture_frame() {
                    if let Ok(encoded) = encoder.encode_frame(&raw_frame) {
                        nal_id += 1;
                        let frags = VideoDatagram::fragment(
                            nal_id, encoded.is_keyframe, encoded.pts_us,
                            &encoded.data, max_frag,
                        );
                        for frag in &frags {
                            let _ = video_tx.try_send(
                                Frame::new(ChannelId::Video, frag.encode()),
                            );
                        }
                    }
                }
            }

            _ = ping_timer.tick() => {
                ping_seq += 1;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64;
                let _ = to_gateway.send(ControlMessage::Ping { seq: ping_seq, timestamp_ms: now }.to_frame()).await;
            }

            msg = from_gateway.recv() => {
                match msg {
                    Some(frame) => {
                        if let Ok(Message::Control(ctrl)) = Message::from_frame(&frame) {
                            handle_control(ctrl, &to_gateway, &mut resize_handler, &mut capture, &mut encoder).await;
                        } else if let Ok(Message::Input(input_msg)) = Message::from_frame(&frame) {
                            let _ = input_backend.inject(&input_msg);
                        }
                    }
                    None => break Ok(()),
                }
            }
        }
    };

    drop(video_tx);
    let _ = video_task.await;
    result
}

/// Handle control messages in test session mode.
pub async fn handle_control(
    msg: ControlMessage,
    to_gateway: &mpsc::Sender<Frame>,
    resize_handler: &mut resize::ResizeHandler,
    capture: &mut dyn CaptureBackend,
    encoder: &mut dyn EncodeBackend,
) {
    match msg {
        ControlMessage::ResolutionRequest { width, height } => {
            let _ = resize_handler.apply(width as u32, height as u32, capture, encoder);
            let ack = ControlMessage::ResolutionAck { width, height };
            let _ = to_gateway.send(ack.to_frame()).await;
        }
        ControlMessage::Ping { seq, timestamp_ms } => {
            let _ = to_gateway
                .send(ControlMessage::Pong { seq, timestamp_ms }.to_frame())
                .await;
        }
        _ => {}
    }
}
