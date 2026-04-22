use std::sync::Arc;
use std::time::Duration;

use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use tower::ServiceExt;

use super::*;
use crate::auth::AuthValidator;
use crate::connect_ticket::SessionConnectTicketManager;
use crate::runtime_manager::{RuntimeManagerConfig, SessionRuntimeManager};

fn test_router() -> (Router, String) {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        session_store: SessionStore::in_memory(),
        runtime_manager: Arc::new(
            SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
                agent_socket_path: "/tmp/test.sock".to_string(),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state), token)
}

fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

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
                        "integration_context": { "ticket": "BPANE-6" }
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
async fn rejects_second_active_session_on_legacy_runtime() {
    let (app, token) = test_router();
    let request_body = json!({
        "viewport": { "width": 1280, "height": 720 }
    })
    .to_string();

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(request_body.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::CREATED);

    let second = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/sessions")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(request_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(second.status(), StatusCode::CONFLICT);
    let body = response_json(second).await;
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("only supports 1 active runtime-backed session"));
}

#[tokio::test]
async fn scopes_session_resources_to_the_authenticated_owner() {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![9; 32]));
    let alpha_token = auth_validator.generate_token().unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let bravo_token = auth_validator.generate_token().unwrap();
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        session_store: SessionStore::in_memory(),
        runtime_manager: Arc::new(
            SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
                agent_socket_path: "/tmp/test.sock".to_string(),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    let app = build_api_router(state);

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&alpha_token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let lookup = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}"))
                .header("authorization", bearer(&bravo_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(lookup.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_session_scoped_runtime_routes_for_unknown_or_foreign_sessions_before_runtime_work()
{
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![11; 32]));
    let alpha_token = auth_validator.generate_token().unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;
    let bravo_token = auth_validator.generate_token().unwrap();
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        session_store: SessionStore::in_memory(),
        runtime_manager: Arc::new(
            SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
                agent_socket_path: "/tmp/test.sock".to_string(),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    let app = build_api_router(state);

    let created = response_json(
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/sessions")
                    .header("authorization", bearer(&alpha_token))
                    .header("content-type", "application/json")
                    .body(Body::from(json!({}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap(),
    )
    .await;
    let session_id = created["id"].as_str().unwrap().to_string();

    let foreign_status = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("authorization", bearer(&bravo_token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(foreign_status.status(), StatusCode::NOT_FOUND);

    let unknown_owner = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/mcp-owner"))
                .header("authorization", bearer(&bravo_token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "width": 1280, "height": 720 }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unknown_owner.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn rejects_session_scoped_runtime_routes_for_stopped_sessions() {
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

    let status_response = app
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/sessions/{session_id}/status"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_response.status(), StatusCode::CONFLICT);
    let body = response_json(status_response).await;
    assert!(body["error"]
        .as_str()
        .unwrap()
        .contains("runtime-compatible state"));
}

#[tokio::test]
async fn owner_can_set_and_clear_session_automation_delegate() {
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

    let delegated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/v1/sessions/{session_id}/automation-owner"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "client_id": "bpane-mcp-bridge",
                        "issuer": "https://issuer.example",
                        "display_name": "BrowserPane MCP bridge"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delegated.status(), StatusCode::OK);
    let delegated_body = response_json(delegated).await;
    assert_eq!(
        delegated_body["automation_delegate"]["client_id"],
        "bpane-mcp-bridge"
    );

    let cleared = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/v1/sessions/{session_id}/automation-owner"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cleared.status(), StatusCode::OK);
    let cleared_body = response_json(cleared).await;
    assert!(cleared_body["automation_delegate"].is_null());
}
