use super::*;
use crate::session_hub::SessionTerminationReason;

#[tokio::test]
async fn explicit_kill_route_stops_session_and_terminates_live_clients() {
    let (app, token, state, agent_server) = test_router_with_live_agent_state().await;

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

    let hub = state
        .registry
        .ensure_hub_for_session(session_uuid, &agent_server.socket_path())
        .await
        .unwrap();
    let live_client = hub.subscribe().await.unwrap();
    hub.set_mcp_owner(1280, 720).await;

    let kill_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/kill"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(kill_response.status(), StatusCode::OK);
    let killed = response_json(kill_response).await;
    assert_eq!(killed["state"], "stopped");
    assert_eq!(killed["status"]["runtime_state"], "stopped");
    assert_eq!(killed["status"]["presence_state"], "empty");
    assert_eq!(killed["status"]["stop_eligibility"]["allowed"], true);
    assert!(state
        .registry
        .telemetry_snapshot_if_live(session_uuid)
        .await
        .is_none());
    assert_eq!(
        live_client.termination_rx.await.unwrap(),
        SessionTerminationReason::SessionKilled
    );
}

#[tokio::test]
async fn kill_route_cancels_active_session_workloads() {
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

    let standalone_task = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/automation-tasks")
                    .header("authorization", bearer(&token))
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "display_name": "kill-standalone-task",
                            "executor": "playwright",
                            "session": {
                                "existing_session_id": session_id,
                            },
                            "input": {
                                "step": "kill-me",
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let standalone_task_id = Uuid::parse_str(standalone_task["id"].as_str().unwrap()).unwrap();

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
                            "name": "kill-workflow",
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
                        "entrypoint": "workflows/kill.ts"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(workflow_version.status(), StatusCode::CREATED);

    let workflow_run = response_json(
        app.clone()
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
            .unwrap(),
    )
    .await;
    let workflow_run_id = Uuid::parse_str(workflow_run["id"].as_str().unwrap()).unwrap();

    let kill_response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/kill"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(kill_response.status(), StatusCode::OK);
    let killed = response_json(kill_response).await;
    assert_eq!(killed["state"], "stopped");

    let owner = AuthenticatedPrincipal {
        subject: created["owner"]["subject"].as_str().unwrap().to_string(),
        issuer: created["owner"]["issuer"].as_str().unwrap().to_string(),
        display_name: None,
        client_id: None,
    };
    let standalone_task = state
        .session_store
        .get_automation_task_for_owner(&owner, standalone_task_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(standalone_task.session_id, session_uuid);
    assert_eq!(standalone_task.state.as_str(), "cancelled");

    let workflow_run = state
        .session_store
        .get_workflow_run_for_owner(&owner, workflow_run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(workflow_run.session_id, session_uuid);
    assert_eq!(workflow_run.state.as_str(), "cancelled");
}
