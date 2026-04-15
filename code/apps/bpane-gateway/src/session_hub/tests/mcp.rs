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
    let browser = hub.subscribe().await.unwrap();

    hub.set_mcp_owner(1920, 1080).await;
    hub.clear_mcp_owner().await;
    assert!(!hub.mcp_is_owner());

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
    assert_eq!(browser.locked_resolution, Some((1280, 720)));
}
