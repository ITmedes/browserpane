use std::sync::Arc;

use super::*;
use crate::session_hub::BrowserClientRole;

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
    assert!(matches!(
        hub.request_resize(owner.client_id, 1440, 900).await,
        ResizeResult::Applied
    ));

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let mut late_joiner = hub.subscribe().await.unwrap();

    assert!(owner.is_owner);
    assert!(late_joiner.is_owner);
    assert!(hub.is_browser_owner(owner.client_id));
    assert!(hub.is_browser_owner(late_joiner.client_id));
    assert_eq!(
        late_joiner.initial_access_state,
        Some(ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::RESIZE_LOCKED,
            width: 1440,
            height: 900,
        })
    );
    match hub.request_resize(late_joiner.client_id, 1280, 720).await {
        ResizeResult::Locked(width, height) => assert_eq!((width, height), (1440, 900)),
        ResizeResult::Applied => panic!("expected Locked"),
    }
    expect_control_message_eventually(
        &mut late_joiner.control_rx,
        ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::RESIZE_LOCKED,
            width: 1440,
            height: 900,
        },
    )
    .await;

    assert!(matches!(
        hub.request_resize(owner.client_id, 1600, 900).await,
        ResizeResult::Applied
    ));
    expect_control_message_eventually(
        &mut late_joiner.control_rx,
        ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::RESIZE_LOCKED,
            width: 1600,
            height: 900,
        },
    )
    .await;

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
    assert_eq!(
        viewer.initial_access_state,
        Some(ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::VIEW_ONLY | ClientAccessFlags::RESIZE_LOCKED,
            width: 1920,
            height: 1080,
        })
    );
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
    let mut viewer = hub.subscribe().await.unwrap();

    assert!(owner.is_owner);
    assert!(!viewer.is_owner);
    assert!(!hub.is_browser_owner(viewer.client_id));

    hub.unsubscribe(owner.client_id).await;

    assert_eq!(hub.client_count(), 1);
    assert!(hub.is_browser_owner(viewer.client_id));
    expect_control_message_eventually(
        &mut viewer.control_rx,
        ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::empty(),
            width: 0,
            height: 0,
        },
    )
    .await;
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

#[tokio::test]
async fn recorder_clients_stay_passive_and_do_not_consume_viewer_slots() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = Arc::new(SessionHub::new(sock_str, 1, true).await.unwrap());

    let owner = hub.subscribe().await.unwrap();
    assert!(matches!(
        hub.request_resize(owner.client_id, 1600, 900).await,
        ResizeResult::Applied
    ));
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    let recorder = hub
        .subscribe_with_role(BrowserClientRole::Recorder)
        .await
        .unwrap();
    assert!(!recorder.is_owner);
    assert_eq!(recorder.client_role, BrowserClientRole::Recorder);
    assert!(!hub.is_browser_owner(recorder.client_id));
    assert_eq!(
        recorder.initial_access_state,
        Some(ControlMessage::ClientAccessState {
            flags: ClientAccessFlags::VIEW_ONLY | ClientAccessFlags::RESIZE_LOCKED,
            width: 1600,
            height: 900,
        })
    );

    let viewer = hub.subscribe().await.unwrap();
    assert!(!viewer.is_owner);

    let err = hub.subscribe().await.unwrap_err();
    assert_eq!(err, SubscribeError::ViewerLimitReached { max_viewers: 1 });

    let snapshot = hub.telemetry_snapshot().await;
    assert_eq!(snapshot.browser_clients, 3);
    assert_eq!(snapshot.viewer_clients, 1);
    assert_eq!(snapshot.recorder_clients, 1);
    assert_eq!(snapshot.viewer_slots_remaining, 0);
}
