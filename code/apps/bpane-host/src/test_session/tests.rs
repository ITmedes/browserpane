use tokio::sync::mpsc;

use bpane_protocol::ControlMessage;

use crate::capture::TestCaptureBackend;
use crate::encode::TestEncoder;
use crate::resize;

#[tokio::test]
async fn handle_control_test_ping_pong() {
    let (tx, mut rx) = mpsc::channel(16);
    let mut resize = resize::ResizeHandler::new(640, 480);
    let mut capture = TestCaptureBackend::new(640, 480);
    let mut encoder = TestEncoder::new(640, 480);

    let ping = ControlMessage::Ping {
        seq: 42,
        timestamp_ms: 1000,
    };
    super::handle_control(ping, &tx, &mut resize, &mut capture, &mut encoder).await;

    let response = rx.recv().await.unwrap();
    let msg = ControlMessage::decode(&response.payload).unwrap();
    assert!(matches!(
        msg,
        ControlMessage::Pong {
            seq: 42,
            timestamp_ms: 1000
        }
    ));
}

#[tokio::test]
async fn handle_control_test_resize() {
    let (tx, mut rx) = mpsc::channel(16);
    let mut resize = resize::ResizeHandler::new(640, 480);
    let mut capture = TestCaptureBackend::new(640, 480);
    let mut encoder = TestEncoder::new(640, 480);

    let req = ControlMessage::ResolutionRequest {
        width: 1920,
        height: 1080,
    };
    super::handle_control(req, &tx, &mut resize, &mut capture, &mut encoder).await;

    let response = rx.recv().await.unwrap();
    let msg = ControlMessage::decode(&response.payload).unwrap();
    assert!(matches!(
        msg,
        ControlMessage::ResolutionAck {
            width: 1920,
            height: 1080
        }
    ));
}

#[tokio::test]
async fn handle_control_test_keyboard_layout_info() {
    let (tx, mut rx) = mpsc::channel(16);
    let mut resize = resize::ResizeHandler::new(640, 480);
    let mut capture = TestCaptureBackend::new(640, 480);
    let mut encoder = TestEncoder::new(640, 480);

    let mut layout_hint = [0u8; 32];
    layout_hint[..2].copy_from_slice(b"fr");
    let msg = ControlMessage::KeyboardLayoutInfo { layout_hint };
    super::handle_control(msg, &tx, &mut resize, &mut capture, &mut encoder).await;

    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn handle_control_test_keyboard_layout_info_empty() {
    let (tx, mut rx) = mpsc::channel(16);
    let mut resize = resize::ResizeHandler::new(640, 480);
    let mut capture = TestCaptureBackend::new(640, 480);
    let mut encoder = TestEncoder::new(640, 480);

    let msg = ControlMessage::KeyboardLayoutInfo {
        layout_hint: [0u8; 32],
    };
    super::handle_control(msg, &tx, &mut resize, &mut capture, &mut encoder).await;

    assert!(rx.try_recv().is_err());
}
