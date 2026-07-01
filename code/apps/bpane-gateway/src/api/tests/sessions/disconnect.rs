use crate::session_control::{
    PersistCompletedSessionRecordingRequest, SessionRecordingFormat,
    SessionRecordingTerminationReason,
};
use crate::session_hub::BrowserClientRole;

use super::*;

#[tokio::test]
async fn session_status_reports_live_connection_descriptors() {
    let (app, token, state, agent_server) = test_router_with_live_agent_state().await;

    let created = create_session_via_api(&app, &token).await;
    let session_id = created["id"].as_str().unwrap().to_string();
    let session_uuid = uuid::Uuid::parse_str(&session_id).unwrap();
    let hub = state
        .registry
        .ensure_hub_for_session(session_uuid, &agent_server.socket_path())
        .await
        .unwrap();

    let owner = hub.subscribe().await.unwrap();
    let recorder = hub
        .subscribe_with_role(BrowserClientRole::Recorder)
        .await
        .unwrap();

    let status_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_response.status(), StatusCode::OK);
    let status = response_json(status_response).await;
    let connections = status["connections"].as_array().unwrap();
    assert_eq!(connections.len(), 2);

    let owner_entry = connections
        .iter()
        .find(|entry| entry["connection_id"] == owner.client_id)
        .unwrap();
    assert_eq!(owner_entry["role"], "owner");

    let recorder_entry = connections
        .iter()
        .find(|entry| entry["connection_id"] == recorder.client_id)
        .unwrap();
    assert_eq!(recorder_entry["role"], "recorder");
}

#[tokio::test]
async fn explicit_disconnect_route_terminates_selected_client_and_updates_status() {
    let (app, token, state, agent_server) = test_router_with_live_agent_state().await;

    let created = create_session_via_api(&app, &token).await;
    let session_id = created["id"].as_str().unwrap().to_string();
    let session_uuid = uuid::Uuid::parse_str(&session_id).unwrap();
    let hub = state
        .registry
        .ensure_hub_for_session(session_uuid, &agent_server.socket_path())
        .await
        .unwrap();

    let mut owner = hub.subscribe().await.unwrap();
    let collaborator = hub.subscribe().await.unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/connections/{}/disconnect",
                    collaborator.client_id
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let disconnected = response_json(response).await;
    assert_eq!(disconnected["connection_counts"]["total_clients"], 1);
    assert_eq!(disconnected["presence_state"], "connected");
    let remaining_connections = disconnected["connections"].as_array().unwrap();
    assert_eq!(remaining_connections.len(), 1);
    assert_eq!(remaining_connections[0]["connection_id"], owner.client_id);

    assert_eq!(
        collaborator.termination_rx.await.unwrap(),
        crate::session_hub::SessionTerminationReason::DisconnectedByOwner
    );
    assert!(matches!(
        owner.termination_rx.try_recv(),
        Err(tokio::sync::oneshot::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn disconnect_all_route_marks_last_live_attachment_idle() {
    let (app, token, state, agent_server) = test_router_with_live_agent_state().await;

    let created = create_session_via_api(&app, &token).await;
    let session_id = created["id"].as_str().unwrap().to_string();
    let session_uuid = uuid::Uuid::parse_str(&session_id).unwrap();
    let hub = state
        .registry
        .ensure_hub_for_session(session_uuid, &agent_server.socket_path())
        .await
        .unwrap();

    let owner = hub.subscribe().await.unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/connections/disconnect-all"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let disconnected = response_json(response).await;
    assert_eq!(disconnected["state"], "idle");
    assert_eq!(disconnected["presence_state"], "idle");
    assert_eq!(disconnected["connection_counts"]["total_clients"], 0);
    assert!(disconnected["connections"].as_array().unwrap().is_empty());
    assert!(disconnected["idle"]["idle_since"].is_string());
    assert!(disconnected["idle"]["idle_deadline"].is_string());

    assert_eq!(
        owner.termination_rx.await.unwrap(),
        crate::session_hub::SessionTerminationReason::DisconnectedAllByOwner
    );
}

#[tokio::test]
async fn disconnect_all_route_finalizes_active_recording() {
    let (app, token, state) =
        test_router_with_recording_lifecycle(recording_lifecycle_test_config());

    let created = create_session_via_api(&app, &token).await;
    let session_id = created["id"].as_str().unwrap().to_string();
    let session_uuid = uuid::Uuid::parse_str(&session_id).unwrap();
    let recording = state
        .session_store
        .create_recording_for_session(session_uuid, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();

    let completion_store = state.session_store.clone();
    let recording_id = recording.id;
    tokio::spawn(async move {
        for _ in 0..100 {
            let current = completion_store
                .get_recording_for_session(session_uuid, recording_id)
                .await
                .unwrap()
                .unwrap();
            if matches!(current.state, StoredSessionRecordingState::Finalizing) {
                let _ = completion_store
                    .complete_recording_for_session(
                        session_uuid,
                        recording_id,
                        PersistCompletedSessionRecordingRequest {
                            artifact_ref: "local_fs:test/disconnect-recording.webm".to_string(),
                            mime_type: Some("video/webm".to_string()),
                            bytes: Some(64),
                            duration_ms: Some(1200),
                        },
                    )
                    .await;
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/sessions/{session_id}/connections/disconnect-all"
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let disconnected = response_json(response).await;
    assert_eq!(disconnected["state"], "idle");

    let finalized = state
        .session_store
        .get_recording_for_session(session_uuid, recording.id)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        finalized.state,
        StoredSessionRecordingState::Ready
    ));
    assert_eq!(
        finalized.termination_reason,
        Some(SessionRecordingTerminationReason::DisconnectAll)
    );
    assert_eq!(finalized.bytes, Some(64));
}

async fn create_session_via_api(app: &Router, token: &str) -> serde_json::Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "template_id": "default",
                        "viewport": { "width": 1280, "height": 720 },
                        "idle_timeout_sec": 300
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await
}
