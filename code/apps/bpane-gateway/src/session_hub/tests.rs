use std::sync::Arc;

use bpane_protocol::channel::ChannelId;
use bpane_protocol::frame::FrameDecoder;
use bpane_protocol::{ControlMessage, SessionFlags, TileMessage};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

use super::{ResizeResult, SessionHub, SubscribeError};

/// Create a mock agent that echoes frames back and responds to
/// ResolutionRequest with ResolutionAck.
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
                        }
                    }
                    Ok(None) => break,
                    Err(e) => panic!("decode error: {e}"),
                }
            }
        }
    })
}

#[tokio::test]
async fn first_subscriber_is_owner() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, true).await.unwrap();
    let c1 = hub.subscribe().await.unwrap();
    assert!(c1.is_owner);
    assert_eq!(c1.client_id, 1);

    let c2 = hub.subscribe().await.unwrap();
    assert!(!c2.is_owner);
    assert_eq!(c2.client_id, 2);
}

#[tokio::test]
async fn collaborative_mode_keeps_late_joiners_interactive() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());
    let c1 = hub.subscribe().await.unwrap();
    let c2 = hub.subscribe().await.unwrap();

    assert!(c1.is_owner);
    assert!(c2.is_owner);
    assert!(hub.is_browser_owner(c1.client_id));
    assert!(hub.is_browser_owner(c2.client_id));

    let result = hub.request_resize(c2.client_id, 1440, 900).await;
    assert!(matches!(result, ResizeResult::Applied));

    let snapshot = hub.telemetry_snapshot().await;
    assert_eq!(snapshot.viewer_clients, 0);
    assert!(!snapshot.exclusive_browser_owner);
}

#[tokio::test]
async fn non_owner_resize_is_denied() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, true).await.unwrap());

    let c1 = hub.subscribe().await.unwrap();
    let result = hub.request_resize(c1.client_id, 1920, 1080).await;
    assert!(matches!(result, ResizeResult::Applied));

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let c2 = hub.subscribe().await.unwrap();
    let result = hub.request_resize(c2.client_id, 800, 600).await;
    match result {
        ResizeResult::Locked(w, h) => {
            assert_eq!(w, 1920);
            assert_eq!(h, 1080);
        }
        _ => panic!("expected Locked"),
    }
}

#[tokio::test]
async fn broadcast_reaches_all_subscribers() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    let mut c1 = hub.subscribe().await.unwrap();
    let mut c2 = hub.subscribe().await.unwrap();

    let timeout = std::time::Duration::from_secs(2);
    let f1 = tokio::time::timeout(timeout, c1.from_host.recv()).await;
    let f2 = tokio::time::timeout(timeout, c2.from_host.recv()).await;

    assert!(f1.is_ok());
    assert!(f2.is_ok());
}

#[tokio::test]
async fn mcp_owner_blocks_browser_resize() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());
    let c1 = hub.subscribe().await.unwrap();

    hub.set_mcp_owner(1920, 1080).await;
    assert!(hub.mcp_is_owner());

    let result = hub.request_resize(c1.client_id, 800, 600).await;
    match result {
        ResizeResult::Locked(w, h) => {
            assert_eq!(w, 1920);
            assert_eq!(h, 1080);
        }
        _ => panic!("expected Locked when MCP is owner"),
    }
}

#[tokio::test]
async fn clear_mcp_owner_restores_normal_behavior() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());
    let c1 = hub.subscribe().await.unwrap();

    hub.set_mcp_owner(1920, 1080).await;
    assert!(hub.mcp_is_owner());

    hub.clear_mcp_owner().await;
    assert!(!hub.mcp_is_owner());

    let result = hub.request_resize(c1.client_id, 800, 600).await;
    assert!(matches!(result, ResizeResult::Applied));
}

#[tokio::test]
async fn subscriber_under_mcp_is_not_owner() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();

    hub.set_mcp_owner(1280, 720).await;

    let c1 = hub.subscribe().await.unwrap();
    assert!(!c1.is_owner);
    assert_eq!(c1.locked_resolution, Some((1280, 720)));
}

#[tokio::test]
async fn late_joiner_gets_cached_session_ready() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    let _c1 = hub.subscribe().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let c2 = hub.subscribe().await.unwrap();
    assert!(
        !c2.initial_frames.is_empty(),
        "late joiner should get cached initial frames"
    );

    let first = &c2.initial_frames[0];
    assert_eq!(first.channel, ChannelId::Control);
}

#[tokio::test]
async fn hub_reports_active_while_agent_connected() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    assert!(hub.is_active());
}

#[tokio::test]
async fn current_resolution_updates_after_resize() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());

    assert_eq!(hub.current_resolution().await, (0, 0));

    let c1 = hub.subscribe().await.unwrap();
    let result = hub.request_resize(c1.client_id, 1280, 720).await;
    assert!(matches!(result, ResizeResult::Applied));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;

    let (w, h) = hub.current_resolution().await;
    assert_eq!((w, h), (1280, 720));
}

#[tokio::test]
async fn client_count_tracks_subscribe_unsubscribe() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    assert_eq!(hub.client_count(), 0);

    let c1 = hub.subscribe().await.unwrap();
    assert_eq!(hub.client_count(), 1);

    let c2 = hub.subscribe().await.unwrap();
    assert_eq!(hub.client_count(), 2);

    hub.unsubscribe(c1.client_id).await;
    assert_eq!(hub.client_count(), 1);

    hub.unsubscribe(c2.client_id).await;
    assert_eq!(hub.client_count(), 0);
}

#[tokio::test]
async fn owner_disconnect_promotes_existing_viewer() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, true).await.unwrap());
    let owner = hub.subscribe().await.unwrap();
    let viewer = hub.subscribe().await.unwrap();

    assert!(owner.is_owner);
    assert!(!viewer.is_owner);
    assert!(!hub.is_browser_owner(viewer.client_id));

    hub.unsubscribe(owner.client_id).await;

    assert_eq!(hub.client_count(), 1);
    assert!(hub.is_browser_owner(viewer.client_id));
    let result = hub.request_resize(viewer.client_id, 1024, 768).await;
    assert!(matches!(result, ResizeResult::Applied));
}

#[tokio::test]
async fn viewer_cap_blocks_extra_viewers() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 1, true).await.unwrap();
    let owner = hub.subscribe().await.unwrap();
    assert!(owner.is_owner);

    let viewer = hub.subscribe().await.unwrap();
    assert!(!viewer.is_owner);

    let err = hub.subscribe().await.unwrap_err();
    assert_eq!(err, SubscribeError::ViewerLimitReached { max_viewers: 1 });
    assert_eq!(hub.client_count(), 2);

    let snapshot = hub.telemetry_snapshot().await;
    assert_eq!(snapshot.viewer_clients, 1);
    assert_eq!(snapshot.joins_rejected_viewer_cap, 1);
    assert_eq!(snapshot.viewer_slots_remaining, 0);
}

#[tokio::test]
async fn telemetry_tracks_join_latency_and_refresh_bursts() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, true).await.unwrap();

    let _owner = hub.subscribe().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let grid = TileMessage::GridConfig {
        tile_size: 64,
        cols: 3,
        rows: 2,
        screen_w: 192,
        screen_h: 128,
    }
    .to_frame();
    *hub.cached_grid_config.lock().await = Some(Arc::new(grid));

    let _viewer = hub.subscribe().await.unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let snapshot = hub.telemetry_snapshot().await;
    assert_eq!(snapshot.browser_clients, 2);
    assert_eq!(snapshot.viewer_clients, 1);
    assert_eq!(snapshot.joins_accepted, 2);
    assert!(snapshot.max_join_latency_ms >= snapshot.last_join_latency_ms);
    assert_eq!(snapshot.full_refresh_requests, 1);
    assert_eq!(snapshot.full_refresh_tiles_requested, 6);
    assert_eq!(snapshot.last_full_refresh_tiles, 6);
    assert_eq!(snapshot.max_full_refresh_tiles, 6);
}
