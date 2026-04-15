use std::sync::Arc;

use super::*;

#[tokio::test]
async fn broadcast_reaches_all_subscribers() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    let mut first = hub.subscribe().await.unwrap();
    let mut second = hub.subscribe().await.unwrap();

    let timeout = std::time::Duration::from_secs(2);
    assert!(tokio::time::timeout(timeout, first.from_host.recv())
        .await
        .is_ok());
    assert!(tokio::time::timeout(timeout, second.from_host.recv())
        .await
        .is_ok());
}

#[tokio::test]
async fn late_joiner_gets_cached_session_ready() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    let _owner = hub.subscribe().await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let late_joiner = hub.subscribe().await.unwrap();
    assert!(
        !late_joiner.initial_frames.is_empty(),
        "late joiner should get cached initial frames"
    );
    assert_eq!(late_joiner.initial_frames[0].channel, ChannelId::Control);
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

    let browser = hub.subscribe().await.unwrap();
    assert!(matches!(
        hub.request_resize(browser.client_id, 1280, 720).await,
        ResizeResult::Applied
    ));

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert_eq!(hub.current_resolution().await, (1280, 720));
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

    let first = hub.subscribe().await.unwrap();
    assert_eq!(hub.client_count(), 1);

    let second = hub.subscribe().await.unwrap();
    assert_eq!(hub.client_count(), 2);

    hub.unsubscribe(first.client_id).await;
    assert_eq!(hub.client_count(), 1);

    hub.unsubscribe(second.client_id).await;
    assert_eq!(hub.client_count(), 0);
}
