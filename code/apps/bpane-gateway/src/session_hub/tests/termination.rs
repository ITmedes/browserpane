use super::*;
use crate::session_hub::SessionTerminationReason;

#[tokio::test]
async fn terminate_all_clients_notifies_live_subscribers() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    let owner = hub.subscribe().await.unwrap();
    let collaborator = hub.subscribe().await.unwrap();

    assert_eq!(
        hub.terminate_all_clients(SessionTerminationReason::SessionKilled)
            .await,
        2
    );
    assert_eq!(
        owner.termination_rx.await.unwrap(),
        SessionTerminationReason::SessionKilled
    );
    assert_eq!(
        collaborator.termination_rx.await.unwrap(),
        SessionTerminationReason::SessionKilled
    );
}

#[tokio::test]
async fn terminate_client_targets_only_the_requested_subscriber() {
    let dir = tempfile::tempdir().unwrap();
    let sock = dir.path().join("test.sock");
    let sock_str = sock.to_str().unwrap();

    let _agent = mock_agent(sock_str).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    let hub = SessionHub::new(sock_str, 10, false).await.unwrap();
    let mut owner = hub.subscribe().await.unwrap();
    let collaborator = hub.subscribe().await.unwrap();

    assert!(
        hub.terminate_client(
            collaborator.client_id,
            SessionTerminationReason::SessionKilled
        )
        .await
    );
    assert_eq!(
        collaborator.termination_rx.await.unwrap(),
        SessionTerminationReason::SessionKilled
    );
    assert!(matches!(
        owner.termination_rx.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
}
