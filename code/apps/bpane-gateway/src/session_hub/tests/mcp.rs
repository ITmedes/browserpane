use std::sync::Arc;

use super::*;

#[tokio::test]
async fn mcp_owner_blocks_browser_resize() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());
    let browser = hub.subscribe().await.unwrap();

    hub.set_mcp_owner(1920, 1080).await;
    assert!(hub.mcp_is_owner());

    match hub.request_resize(browser.client_id, 800, 600).await {
        ResizeResult::Locked(width, height) => assert_eq!((width, height), (1920, 1080)),
        ResizeResult::Applied => panic!("expected Locked when MCP is owner"),
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
    let mut browser = hub.subscribe().await.unwrap();

    hub.set_mcp_owner(1920, 1080).await;
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(1), browser.control_rx.recv(),)
            .await
            .unwrap(),
        Some(ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::VIEW_ONLY | ClientAccessFlags::RESIZE_LOCKED,
            width: 1920,
            height: 1080,
        })
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
async fn subscriber_under_mcp_is_not_owner() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    hub.set_mcp_owner(1280, 720).await;

    let browser = hub.subscribe().await.unwrap();
    assert!(!browser.is_owner);
    assert_eq!(
        browser.initial_access_state,
        Some(ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::VIEW_ONLY | ClientAccessFlags::RESIZE_LOCKED,
            width: 1280,
            height: 720,
        })
    );
}
