use super::*;
use crate::session_control::{
    SessionConnectionCounts, SessionIdleStatus, SessionPresenceState, SessionRuntimeState,
    SessionStatusSummary, SessionStopBlocker, SessionStopBlockerKind, SessionStopEligibility,
};

#[tokio::test]
async fn rejects_v1_session_routes_without_bearer_auth() {
    let (app, _) = test_router();
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[test]
fn session_status_maps_recorder_clients() {
    let latest_recording = StoredSessionRecording {
        id: uuid::Uuid::now_v7(),
        session_id: uuid::Uuid::now_v7(),
        previous_recording_id: None,
        state: StoredSessionRecordingState::Recording,
        format: SessionRecordingFormat::Webm,
        mime_type: Some("video/webm".to_string()),
        bytes: Some(4096),
        duration_ms: Some(1200),
        error: None,
        termination_reason: None,
        artifact_ref: None,
        started_at: chrono::Utc::now(),
        completed_at: None,
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };
    let playback =
        prepare_session_recording_playback(latest_recording.session_id, &[], chrono::Utc::now());
    let status = session_status_from_snapshot(
        SessionLifecycleState::Active,
        SessionStatusSummary {
            runtime_state: SessionRuntimeState::Running,
            presence_state: SessionPresenceState::Connected,
            connection_counts: SessionConnectionCounts {
                interactive_clients: 2,
                owner_clients: 1,
                viewer_clients: 1,
                recorder_clients: 1,
                automation_clients: 0,
                total_clients: 3,
            },
            stop_eligibility: SessionStopEligibility {
                allowed: false,
                blockers: vec![
                    SessionStopBlocker {
                        kind: SessionStopBlockerKind::OwnerClients,
                        count: 1,
                    },
                    SessionStopBlocker {
                        kind: SessionStopBlockerKind::ViewerClients,
                        count: 1,
                    },
                    SessionStopBlocker {
                        kind: SessionStopBlockerKind::RecorderClients,
                        count: 1,
                    },
                ],
            },
            idle: SessionIdleStatus {
                idle_timeout_sec: Some(300),
                idle_since: None,
                idle_deadline: None,
            },
        },
        SessionTelemetrySnapshot {
            browser_clients: 3,
            viewer_clients: 1,
            recorder_clients: 1,
            max_viewers: 10,
            viewer_slots_remaining: 9,
            exclusive_browser_owner: false,
            mcp_owner: false,
            resolution: (1280, 720),
            joins_accepted: 4,
            joins_rejected_viewer_cap: 0,
            last_join_latency_ms: 12,
            average_join_latency_ms: 9.5,
            max_join_latency_ms: 15,
            full_refresh_requests: 1,
            full_refresh_tiles_requested: 30,
            last_full_refresh_tiles: 30,
            max_full_refresh_tiles: 30,
            egress_send_stream_lock_acquires_total: 10,
            egress_send_stream_lock_wait_us_total: 20,
            egress_send_stream_lock_wait_us_average: 2.0,
            egress_send_stream_lock_wait_us_max: 6,
            egress_lagged_receives_total: 0,
            egress_lagged_frames_total: 0,
        },
        &SessionRecordingPolicy {
            mode: SessionRecordingMode::Manual,
            format: SessionRecordingFormat::Webm,
            retention_sec: Some(86_400),
        },
        Some(&latest_recording),
        playback.resource,
    );

    assert_eq!(status.state, SessionLifecycleState::Active);
    assert_eq!(status.summary.runtime_state, SessionRuntimeState::Running);
    assert_eq!(
        status.summary.presence_state,
        SessionPresenceState::Connected
    );
    assert_eq!(status.summary.connection_counts.total_clients, 3);
    assert_eq!(status.summary.connection_counts.owner_clients, 1);
    assert_eq!(status.summary.connection_counts.viewer_clients, 1);
    assert!(!status.summary.stop_eligibility.allowed);
    assert_eq!(status.summary.stop_eligibility.blockers.len(), 3);
    assert_eq!(status.browser_clients, 3);
    assert_eq!(status.viewer_clients, 1);
    assert_eq!(status.recorder_clients, 1);
    assert_eq!(status.viewer_slots_remaining, 9);
    assert_eq!(
        status.recording.configured_mode,
        SessionRecordingMode::Manual
    );
    assert_eq!(status.recording.format, SessionRecordingFormat::Webm);
    assert_eq!(status.recording.retention_sec, Some(86_400));
    assert!(matches!(
        status.recording.state,
        SessionRecordingStatusState::Recording
    ));
    assert!(status.recording.recorder_attached);
    assert!(status.recording.active_recording_id.is_some());
    assert_eq!(status.recording.bytes_written, Some(4096));
    assert_eq!(status.recording.duration_ms, Some(1200));
}

#[tokio::test]
async fn creates_lists_gets_and_stops_a_session_resource() {
    let (app, token) = test_router();

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "template_id": "default",
                        "viewport": { "width": 1440, "height": 900 },
                        "idle_timeout_sec": 900,
                        "labels": { "suite": "contract" },
                        "integration_context": { "ticket": "BPANE-6" },
                        "recording": {
                          "mode": "manual",
                          "format": "webm",
                          "retention_sec": 86400
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created = response_json(create_response).await;
    let session_id = created["id"].as_str().unwrap().to_string();
    assert_eq!(created["state"], "ready");
    assert_eq!(created["owner_mode"], "collaborative");
    assert_eq!(created["idle_timeout_sec"], 900);
    assert_eq!(created["template_id"], "default");
    assert!(created["automation_delegate"].is_null());
    assert_eq!(created["viewport"]["width"], 1440);
    assert_eq!(created["viewport"]["height"], 900);
    assert_eq!(created["capabilities"]["browser_input"], true);
    assert_eq!(created["capabilities"]["clipboard"], true);
    assert_eq!(created["capabilities"]["audio"], true);
    assert_eq!(created["capabilities"]["microphone"], true);
    assert_eq!(created["capabilities"]["camera"], true);
    assert_eq!(created["capabilities"]["file_transfer"], true);
    assert_eq!(created["capabilities"]["resize"], true);
    assert!(created["owner"]["subject"].is_string());
    assert!(created["owner"]["issuer"].is_string());
    assert_eq!(created["labels"]["suite"], "contract");
    assert_eq!(created["integration_context"]["ticket"], "BPANE-6");
    assert_eq!(created["recording"]["mode"], "manual");
    assert_eq!(created["recording"]["format"], "webm");
    assert_eq!(created["recording"]["retention_sec"], 86400);
    assert_eq!(created["connect"]["gateway_url"], "https://localhost:4433");
    assert_eq!(created["connect"]["transport_path"], "/session");
    assert_eq!(created["connect"]["auth_type"], "session_connect_ticket");
    assert_eq!(
        created["connect"]["ticket_path"],
        format!("/api/v1/sessions/{session_id}/access-tokens")
    );
    assert_eq!(
        created["connect"]["compatibility_mode"],
        "legacy_single_runtime"
    );
    assert_eq!(created["runtime"]["binding"], "legacy_single_session");
    assert_eq!(
        created["runtime"]["compatibility_mode"],
        "legacy_single_runtime"
    );
    assert_eq!(created["runtime"]["cdp_endpoint"], "http://host:9223");
    assert_eq!(created["status"]["runtime_state"], "not_started");
    assert_eq!(created["status"]["presence_state"], "empty");
    assert_eq!(
        created["status"]["connection_counts"]["interactive_clients"],
        0
    );
    assert_eq!(created["status"]["connection_counts"]["owner_clients"], 0);
    assert_eq!(created["status"]["connection_counts"]["viewer_clients"], 0);
    assert_eq!(
        created["status"]["connection_counts"]["recorder_clients"],
        0
    );
    assert_eq!(
        created["status"]["connection_counts"]["automation_clients"],
        0
    );
    assert_eq!(created["status"]["connection_counts"]["total_clients"], 0);
    assert_eq!(created["status"]["stop_eligibility"]["allowed"], true);
    assert!(created["status"]["stop_eligibility"]["blockers"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(created["status"]["idle"]["idle_timeout_sec"], 900);
    assert!(created["status"]["idle"]["idle_since"].is_null());
    assert!(created["status"]["idle"]["idle_deadline"].is_null());
    assert!(created["created_at"].is_string());
    assert!(created["updated_at"].is_string());
    assert!(created["stopped_at"].is_null());

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let listed = response_json(list_response).await;
    assert_eq!(listed["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(listed["sessions"][0]["id"], session_id);

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let fetched = response_json(get_response).await;
    assert_eq!(fetched["id"], session_id);
    assert_eq!(fetched["labels"]["suite"], "contract");
    assert_eq!(fetched["recording"]["mode"], "manual");
    assert_eq!(fetched["status"]["runtime_state"], "not_started");
    assert_eq!(fetched["status"]["presence_state"], "empty");

    let issue_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/access-tokens"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(issue_response.status(), StatusCode::OK);
    let issued = response_json(issue_response).await;
    assert_eq!(issued["session_id"], session_id);
    assert_eq!(issued["token_type"], "session_connect_ticket");
    assert!(issued["token"].as_str().unwrap().starts_with("v1."));
    assert!(issued["expires_at"].is_string());
    assert_eq!(issued["connect"]["gateway_url"], "https://localhost:4433");
    assert_eq!(issued["connect"]["transport_path"], "/session");
    assert_eq!(issued["connect"]["auth_type"], "session_connect_ticket");
    assert_eq!(
        issued["connect"]["ticket_path"],
        format!("/api/v1/sessions/{session_id}/access-tokens")
    );
    assert_eq!(
        issued["connect"]["compatibility_mode"],
        "legacy_single_runtime"
    );

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);
    let stopped = response_json(delete_response).await;
    assert_eq!(stopped["id"], session_id);
    assert_eq!(stopped["state"], "stopped");
    assert!(stopped["stopped_at"].is_string());
}

#[tokio::test]
async fn stopped_session_can_issue_a_new_connect_ticket_and_resume() {
    let (app, token) = test_router();

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({ "idle_timeout_sec": 300 }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::OK);

    let issue_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/access-tokens"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(issue_response.status(), StatusCode::OK);
    let issued = response_json(issue_response).await;
    assert_eq!(issued["session_id"], session_id);
    assert_eq!(issued["token_type"], "session_connect_ticket");

    let get_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_response.status(), StatusCode::OK);
    let fetched = response_json(get_response).await;
    assert_eq!(fetched["state"], "ready");
    assert!(fetched["stopped_at"].is_null());
}
