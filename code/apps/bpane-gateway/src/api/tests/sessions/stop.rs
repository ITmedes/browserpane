use super::*;
use crate::session_control::{
    PersistCompletedSessionRecordingRequest, SessionRecordingFormat,
    SessionRecordingTerminationReason,
};

fn blocker_kinds(value: &Value) -> Vec<&str> {
    value["session"]["status"]["stop_eligibility"]["blockers"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|blocker| blocker["kind"].as_str())
        .collect()
}

#[tokio::test]
async fn explicit_stop_route_stops_unused_session() {
    let (app, token) = test_router();

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let stop_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/stop"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(stop_response.status(), StatusCode::OK);
    let stopped = response_json(stop_response).await;
    assert_eq!(stopped["state"], "stopped");
    assert_eq!(stopped["status"]["runtime_state"], "stopped");
    assert_eq!(stopped["status"]["presence_state"], "empty");
    assert_eq!(stopped["status"]["stop_eligibility"]["allowed"], true);
}

#[tokio::test]
async fn explicit_stop_route_finalizes_active_recording_instead_of_conflicting() {
    let (app, token, state) =
        test_router_with_recording_lifecycle(recording_lifecycle_test_config());

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();
    let session_uuid = Uuid::parse_str(&session_id).unwrap();
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
                            artifact_ref: "local_fs:test/stop-recording.webm".to_string(),
                            mime_type: Some("video/webm".to_string()),
                            bytes: Some(128),
                            duration_ms: Some(2000),
                        },
                    )
                    .await;
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }
    });

    let stop_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/stop"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(stop_response.status(), StatusCode::OK);
    let stopped = response_json(stop_response).await;
    assert_eq!(stopped["state"], "stopped");

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
        Some(SessionRecordingTerminationReason::SessionStop)
    );
    assert_eq!(finalized.bytes, Some(128));
}

#[tokio::test]
async fn stop_routes_report_execution_blockers_without_treating_recording_as_a_stop_blocker() {
    let (app, token, state) = test_router_with_state();

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();
    let session_uuid = Uuid::parse_str(&session_id).unwrap();

    let automation_task = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/automation-tasks")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "display_name": "stop-blocker-task",
                        "executor": "playwright",
                        "session": {
                            "existing_session_id": session_id,
                        },
                        "input": {
                            "step": "block-stop",
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(automation_task.status(), StatusCode::CREATED);

    let workflow = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/workflows")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "stop-blocker-workflow",
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let workflow_id = workflow["id"].as_str().unwrap();

    let workflow_version = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/workflows/{workflow_id}/versions"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "version": "v1",
                        "executor": "playwright",
                        "entrypoint": "workflows/stop-blocker.ts"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(workflow_version.status(), StatusCode::CREATED);

    let workflow_run = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/workflow-runs")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "workflow_id": workflow_id,
                        "version": "v1",
                        "session": {
                            "existing_session_id": session_id,
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(workflow_run.status(), StatusCode::CREATED);

    let recording = state
        .session_store
        .create_recording_for_session(session_uuid, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();
    let finalizing = state
        .session_store
        .stop_recording_for_session(
            session_uuid,
            recording.id,
            SessionRecordingTerminationReason::SessionStop,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(finalizing.state.as_str(), "finalizing");

    let session = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/sessions/{session_id}"))
                    .header("authorization", bearer(&token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(session["status"]["stop_eligibility"]["allowed"], false);
    let session_wrapper = json!({ "session": session.clone() });
    let session_blockers = blocker_kinds(&session_wrapper);
    assert!(session_blockers.contains(&"automation_tasks"));
    assert!(session_blockers.contains(&"workflow_runs"));
    assert!(!session_blockers.contains(&"recording_activity"));

    let stop_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/stop"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(stop_response.status(), StatusCode::CONFLICT);
    let stop_body = response_json(stop_response).await;
    assert_eq!(stop_body["session"]["id"], session_id);
    assert_eq!(
        stop_body["session"]["status"]["stop_eligibility"]["allowed"],
        false
    );
    let stop_blockers = blocker_kinds(&stop_body);
    assert!(stop_blockers.contains(&"automation_tasks"));
    assert!(stop_blockers.contains(&"workflow_runs"));
    assert!(!stop_blockers.contains(&"recording_activity"));

    let delete_response = app
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
    assert_eq!(delete_response.status(), StatusCode::CONFLICT);
    let delete_body = response_json(delete_response).await;
    assert_eq!(delete_body["session"]["id"], session_id);
    let delete_blockers = blocker_kinds(&delete_body);
    assert!(delete_blockers.contains(&"automation_tasks"));
    assert!(delete_blockers.contains(&"workflow_runs"));
    assert!(!delete_blockers.contains(&"recording_activity"));
}
