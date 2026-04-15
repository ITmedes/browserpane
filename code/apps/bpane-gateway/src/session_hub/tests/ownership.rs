use std::sync::Arc;

use super::*;

#[tokio::test]
async fn first_subscriber_is_owner() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, true).await.unwrap();
    let owner = hub.subscribe().await.unwrap();
    assert!(owner.is_owner);
    assert_eq!(owner.client_id, 1);

    let viewer = hub.subscribe().await.unwrap();
    assert!(!viewer.is_owner);
    assert_eq!(viewer.client_id, 2);
}

#[tokio::test]
async fn collaborative_mode_keeps_late_joiners_interactive() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 10, false).await.unwrap());
    let owner = hub.subscribe().await.unwrap();
    let late_joiner = hub.subscribe().await.unwrap();

    assert!(owner.is_owner);
    assert!(late_joiner.is_owner);
    assert!(hub.is_browser_owner(owner.client_id));
    assert!(hub.is_browser_owner(late_joiner.client_id));
    assert!(matches!(
        hub.request_resize(late_joiner.client_id, 1440, 900).await,
        ResizeResult::Applied
    ));

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

    let owner = hub.subscribe().await.unwrap();
    assert!(matches!(
        hub.request_resize(owner.client_id, 1920, 1080).await,
        ResizeResult::Applied
    ));

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let viewer = hub.subscribe().await.unwrap();
    match hub.request_resize(viewer.client_id, 800, 600).await {
        ResizeResult::Locked(width, height) => assert_eq!((width, height), (1920, 1080)),
        ResizeResult::Applied => panic!("expected Locked"),
    }
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
    assert!(matches!(
        hub.request_resize(viewer.client_id, 1024, 768).await,
        ResizeResult::Applied
    ));
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
