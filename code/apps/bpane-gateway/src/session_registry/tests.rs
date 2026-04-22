use std::sync::Arc;

use bpane_protocol::ControlMessage;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixListener;
use uuid::Uuid;

use super::SessionRegistry;

fn session_id() -> Uuid {
    Uuid::now_v7()
}

async fn mock_agent(sock_path: &str) -> tokio::task::JoinHandle<()> {
    mock_agent_with_connections(sock_path, 1).await
}

async fn mock_agent_with_connections(
    sock_path: &str,
    expected_connections: usize,
) -> tokio::task::JoinHandle<()> {
    let listener = UnixListener::bind(sock_path).unwrap();
    tokio::spawn(async move {
        let ready = ControlMessage::SessionReady {
            version: 1,
            flags: bpane_protocol::SessionFlags::empty(),
        };
        let encoded = ready.to_frame().encode();
        let mut streams = Vec::with_capacity(expected_connections);
        for _ in 0..expected_connections {
            let (mut stream, _) = listener.accept().await.unwrap();
            stream.write_all(&encoded).await.unwrap();
            streams.push(stream);
        }
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
        drop(streams);
    })
}

#[tokio::test]
async fn two_clients_share_same_hub() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let registry = SessionRegistry::new(10, false);
    let session_id = session_id();

    let (c1, hub1) = registry.join(session_id, sock_str).await.unwrap();
    assert!(c1.is_owner);

    let (c2, hub2) = registry.join(session_id, sock_str).await.unwrap();
    assert!(c2.is_owner);

    assert!(Arc::ptr_eq(&hub1, &hub2));
    assert_eq!(hub1.client_count(), 2);
}

#[tokio::test]
async fn exclusive_owner_mode_marks_late_joiner_as_viewer() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let registry = SessionRegistry::new(10, true);
    let session_id = session_id();

    let (c1, _) = registry.join(session_id, sock_str).await.unwrap();
    let (c2, _) = registry.join(session_id, sock_str).await.unwrap();
    assert!(c1.is_owner);
    assert!(!c2.is_owner);
}

#[tokio::test]
async fn leave_nonexistent_session_does_not_panic() {
    let registry = SessionRegistry::new(10, false);
    registry.leave(session_id(), 42).await;
}

#[tokio::test]
async fn ensure_hub_creates_hub_without_subscribing() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let registry = SessionRegistry::new(10, false);
    let hub = registry
        .ensure_hub_for_session(session_id(), sock_str)
        .await
        .unwrap();
    assert_eq!(hub.client_count(), 0);
    assert!(hub.is_active());
}

#[tokio::test]
async fn ensure_hub_reuses_existing() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let registry = SessionRegistry::new(10, false);
    let session_id = session_id();

    let hub1 = registry
        .ensure_hub_for_session(session_id, sock_str)
        .await
        .unwrap();
    let hub2 = registry
        .ensure_hub_for_session(session_id, sock_str)
        .await
        .unwrap();
    assert!(Arc::ptr_eq(&hub1, &hub2));
}

#[tokio::test]
async fn concurrent_ensure_hub_shares_same_hub() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent_with_connections(sock_str, 2).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let registry = Arc::new(SessionRegistry::new(10, false));
    let registry2 = Arc::clone(&registry);
    let session_id = session_id();

    let (hub1, hub2) = tokio::join!(
        registry.ensure_hub_for_session(session_id, sock_str),
        registry2.ensure_hub_for_session(session_id, sock_str)
    );
    let hub1 = hub1.unwrap();
    let hub2 = hub2.unwrap();

    assert!(Arc::ptr_eq(&hub1, &hub2));
    assert_eq!(hub1.client_count(), 0);
}

#[tokio::test]
async fn join_after_ensure_hub_shares_same_hub() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let registry = SessionRegistry::new(10, false);
    let session_id = session_id();

    let hub_via_ensure = registry
        .ensure_hub_for_session(session_id, sock_str)
        .await
        .unwrap();
    assert_eq!(hub_via_ensure.client_count(), 0);

    let (c1, hub_via_join) = registry.join(session_id, sock_str).await.unwrap();
    assert!(Arc::ptr_eq(&hub_via_ensure, &hub_via_join));
    assert!(c1.is_owner);
    assert_eq!(hub_via_ensure.client_count(), 1);
}

#[tokio::test]
async fn leave_decrements_count() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let registry = SessionRegistry::new(10, false);
    let session_id = session_id();

    let (c1, hub) = registry.join(session_id, sock_str).await.unwrap();
    let (c2, _) = registry.join(session_id, sock_str).await.unwrap();
    assert_eq!(hub.client_count(), 2);

    registry.leave(session_id, c1.client_id).await;
    assert_eq!(hub.client_count(), 1);

    registry.leave(session_id, c2.client_id).await;
    assert_eq!(hub.client_count(), 0);
}
