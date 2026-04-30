use std::sync::Arc;

use bpane_protocol::frame::Frame;

use super::*;
use crate::session_hub::ClientHandle;

#[tokio::test]
async fn mcp_owner_preserves_existing_browser_resize_owner() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());
    let browser = hub.subscribe().await.unwrap();

    hub.set_mcp_owner(1920, 1080).await;
    assert!(hub.mcp_is_owner());

    assert!(browser.is_owner);
    assert!(hub.is_browser_owner(browser.client_id));
    assert!(matches!(
        hub.request_resize(browser.client_id, 800, 600).await,
        ResizeResult::Applied
    ));
}

#[tokio::test]
async fn clear_mcp_owner_restores_normal_behavior_after_mcp_seeded_resolution() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());
    hub.set_mcp_owner(1920, 1080).await;
    let mut browser = hub.subscribe().await.unwrap();
    assert!(browser.is_owner);
    assert_eq!(
        Some(ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::RESIZE_LOCKED,
            width: 1920,
            height: 1080,
        }),
        browser.initial_access_state
    );

    hub.clear_mcp_owner().await;
    assert!(!hub.mcp_is_owner());
    expect_control_message_eventually(
        &mut browser.control_rx,
        ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::empty(),
            width: 0,
            height: 0,
        },
    )
    .await;

    assert!(matches!(
        hub.request_resize(browser.client_id, 800, 600).await,
        ResizeResult::Applied
    ));
}

#[tokio::test]
async fn subscriber_under_mcp_remains_interactive_but_resize_locked_when_mcp_connected_first() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    hub.set_mcp_owner(1280, 720).await;

    let browser = hub.subscribe().await.unwrap();
    assert!(browser.is_owner);
    assert!(hub.is_browser_owner(browser.client_id));
    assert_eq!(
        browser.initial_access_state,
        Some(ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::RESIZE_LOCKED,
            width: 1280,
            height: 720,
        })
    );
    match hub.request_resize(browser.client_id, 800, 600).await {
        ResizeResult::Locked(width, height) => assert_eq!((width, height), (1280, 720)),
        ResizeResult::Applied => panic!("expected Locked while MCP controls resolution"),
    }

    let snapshot = hub.telemetry_snapshot().await;
    assert!(snapshot.mcp_owner);
    assert_eq!(snapshot.viewer_clients, 0);
}

#[tokio::test]
async fn first_browser_under_mcp_gets_full_repaint_after_lock_state() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    hub.set_mcp_owner(1280, 720).await;
    *hub.cached_grid_config.lock().await = Some(Arc::new(
        TileMessage::GridConfig {
            tile_size: 64,
            cols: 2,
            rows: 1,
            screen_w: 128,
            screen_h: 64,
        }
        .to_frame(),
    ));

    let mut browser = hub.subscribe().await.unwrap();
    assert_eq!(
        browser.initial_access_state,
        Some(ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::RESIZE_LOCKED,
            width: 1280,
            height: 720,
        })
    );

    let repaint = next_tile_frame(&mut browser).await;
    assert_eq!(repaint.channel, ChannelId::Tiles);
    assert!(matches!(
        TileMessage::decode(&repaint.payload).unwrap(),
        TileMessage::Fill { .. }
    ));

    let snapshot = hub.telemetry_snapshot().await;
    assert_eq!(snapshot.full_refresh_requests, 1);
    assert_eq!(snapshot.full_refresh_tiles_requested, 2);
}

async fn next_tile_frame(handle: &mut ClientHandle) -> Arc<Frame> {
    for _ in 0..4 {
        let frame =
            tokio::time::timeout(std::time::Duration::from_secs(1), handle.from_host.recv())
                .await
                .unwrap()
                .unwrap();
        if frame.channel == ChannelId::Tiles {
            return frame;
        }
    }

    panic!("did not receive a tile repaint frame");
}
