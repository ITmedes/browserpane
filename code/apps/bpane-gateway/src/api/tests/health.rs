use axum::body::to_bytes;
use serde_json::Value;

use super::*;
use crate::lifecycle::GatewayLifecycle;
use crate::readiness::GatewayReadiness;

#[tokio::test]
async fn health_and_readiness_are_public_and_drain_rejects_new_work() {
    let temp_dir = tempdir().unwrap();
    let socket_path = temp_dir.path().join("agent.sock");
    let _agent_listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
    let lifecycle = Arc::new(GatewayLifecycle::new());
    assert!(lifecycle.mark_running());
    let readiness = Arc::new(GatewayReadiness::new(
        lifecycle.clone(),
        SessionStore::in_memory(),
        Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: socket_path.to_string_lossy().into_owned(),
                cdp_endpoint: None,
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        None,
        Arc::new(RecordingArtifactStore::local_fs(
            temp_dir.path().join("recordings"),
            temp_dir.path().join("recording-staging"),
        )),
        Arc::new(WorkspaceFileStore::local_fs(
            temp_dir.path().join("workspaces"),
        )),
        Duration::from_secs(1),
    ));
    let (_, _, state) = test_router_with_state();
    let app = build_gateway_api_router(state, lifecycle.clone(), readiness);

    let health = app
        .clone()
        .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    let health_json: Value =
        serde_json::from_slice(&to_bytes(health.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(health_json["status"], "live");
    assert_eq!(health_json["lifecycle"], "running");

    let ready = app
        .clone()
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(ready.status(), StatusCode::OK);
    let ready_json: Value =
        serde_json::from_slice(&to_bytes(ready.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(ready_json["status"], "ready");
    assert_eq!(ready_json["checks"].as_array().unwrap().len(), 4);

    assert!(lifecycle.begin_draining());
    let draining_ready = app
        .clone()
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(draining_ready.status(), StatusCode::SERVICE_UNAVAILABLE);
    let rejected = app
        .oneshot(
            Request::get("/api/v1/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::SERVICE_UNAVAILABLE);
}

#[tokio::test]
async fn readiness_failure_is_sanitized() {
    let temp_dir = tempdir().unwrap();
    let lifecycle = Arc::new(GatewayLifecycle::new());
    assert!(lifecycle.mark_running());
    let readiness = Arc::new(GatewayReadiness::new(
        lifecycle.clone(),
        SessionStore::in_memory(),
        Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: temp_dir
                    .path()
                    .join("secret-runtime-path.sock")
                    .to_string_lossy()
                    .into_owned(),
                cdp_endpoint: None,
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        None,
        Arc::new(RecordingArtifactStore::local_fs(
            temp_dir.path().join("recordings"),
            temp_dir.path().join("recording-staging"),
        )),
        Arc::new(WorkspaceFileStore::local_fs(
            temp_dir.path().join("workspaces"),
        )),
        Duration::from_secs(1),
    ));
    let (_, _, state) = test_router_with_state();
    let app = build_gateway_api_router(state, lifecycle, readiness);

    let response = app
        .oneshot(Request::get("/readyz").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body_text = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_text.contains("runtime_manager"));
    assert!(body_text.contains("dependency check failed"));
    assert!(!body_text.contains("secret-runtime-path"));
}
