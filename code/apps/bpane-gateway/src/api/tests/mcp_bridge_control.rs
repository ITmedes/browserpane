use super::*;

#[tokio::test]
async fn mcp_bridge_control_proxy_uses_gateway_auth_and_internal_bearer() {
    let bridge = MockBridge::start().await;
    let (app, token, _state) = test_router_with_mcp_bridge_control(McpBridgeControlConfig {
        control_url: bridge.control_url.clone(),
        bearer_token: Some("internal-token".to_string()),
        timeout: Duration::from_secs(5),
    });

    let unauthenticated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/mcp-bridge/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    assert!(bridge.requests.lock().await.is_empty());

    let health = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/mcp-bridge/health")
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(health.status(), StatusCode::OK);
    assert_eq!(response_json(health).await["status"], "ok");

    let session = response_json(
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
    let session_id = session["id"].as_str().unwrap();

    let delegated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/mcp-bridge/control-session")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "session_id": session_id,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delegated.status(), StatusCode::OK);
    let delegated_body = response_json(delegated).await;
    assert_eq!(delegated_body["session"]["id"], session_id);

    let requests = bridge.requests.lock().await;
    assert!(requests.iter().any(|request| {
        request.method == "GET"
            && request.path == "/health"
            && request.authorization.as_deref() == Some("Bearer internal-token")
    }));
    assert!(requests.iter().any(|request| {
        request.method == "PUT"
            && request.path == "/control-session"
            && request.authorization.as_deref() == Some("Bearer internal-token")
            && request.body.get("session_id").and_then(Value::as_str) == Some(session_id)
    }));
}

#[tokio::test]
async fn mcp_bridge_control_put_reconciles_after_ambiguous_bridge_failure() {
    let bridge = MockBridge::start_with_options(MockBridgeOptions {
        fail_put_after_write: true,
    })
    .await;
    let (app, token, _state) = test_router_with_mcp_bridge_control(McpBridgeControlConfig {
        control_url: bridge.control_url.clone(),
        bearer_token: Some("internal-token".to_string()),
        timeout: Duration::from_secs(5),
    });
    let session = response_json(
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
    let session_id = session["id"].as_str().unwrap();

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/v1/mcp-bridge/control-session")
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(json!({ "session_id": session_id }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["session"]["id"], session_id);
    let requests = bridge.requests.lock().await;
    assert!(requests
        .iter()
        .any(|request| request.method == "PUT" && request.path == "/control-session"));
    assert!(requests
        .iter()
        .any(|request| request.method == "GET" && request.path == "/control-session"));
}

#[derive(Default)]
struct MockBridgeOptions {
    fail_put_after_write: bool,
}

#[derive(Clone)]
struct MockBridgeState {
    requests: Arc<Mutex<Vec<MockBridgeRequest>>>,
    control_session_id: Arc<Mutex<Option<String>>>,
    fail_put_after_write: bool,
}

struct MockBridge {
    control_url: String,
    requests: Arc<Mutex<Vec<MockBridgeRequest>>>,
    shutdown: Option<oneshot::Sender<()>>,
}

#[derive(Debug)]
struct MockBridgeRequest {
    method: String,
    path: String,
    authorization: Option<String>,
    body: Value,
}

impl MockBridge {
    async fn start() -> Self {
        Self::start_with_options(MockBridgeOptions::default()).await
    }

    async fn start_with_options(options: MockBridgeOptions) -> Self {
        let state = MockBridgeState {
            requests: Arc::new(Mutex::new(Vec::new())),
            control_session_id: Arc::new(Mutex::new(None)),
            fail_put_after_write: options.fail_put_after_write,
        };
        let app = axum::Router::new()
            .route(
                "/control-session",
                axum::routing::get(mock_get_control_session)
                    .put(mock_put_control_session)
                    .delete(mock_delete_control_session),
            )
            .route("/health", axum::routing::get(mock_health))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap();
        });
        Self {
            control_url: format!("http://{addr}/control-session"),
            requests: state.requests,
            shutdown: Some(shutdown_tx),
        }
    }
}

impl Drop for MockBridge {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }
}

async fn mock_health(
    headers: HeaderMap,
    State(state): State<MockBridgeState>,
) -> Result<Json<Value>, StatusCode> {
    record_mock_bridge_request(&state, "GET", "/health", headers, json!({})).await;
    Ok(Json(json!({
        "status": "ok",
        "clients": 0,
        "control_session_id": null,
        "control_session_state": null,
        "control_session_backend_delegated": false,
        "bridge_alignment": null,
        "managed_sessions": [],
    })))
}

async fn mock_get_control_session(
    headers: HeaderMap,
    State(state): State<MockBridgeState>,
) -> Result<Json<Value>, StatusCode> {
    authorize_mock_bridge(&headers)?;
    record_mock_bridge_request(&state, "GET", "/control-session", headers, json!({})).await;
    let session_id = state.control_session_id.lock().await.clone();
    Ok(Json(json!({
        "locked": false,
        "session": session_id.as_ref().map(|id| json!({ "id": id })),
        "cdp_endpoint": session_id.as_ref().map(|_| json!("http://browser:9222")),
        "playwright_cdp_endpoint": null,
        "playwright_effective_cdp_endpoint": null,
    })))
}

async fn mock_put_control_session(
    headers: HeaderMap,
    State(state): State<MockBridgeState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    authorize_mock_bridge(&headers)?;
    let session_id = body["session_id"].as_str().unwrap().to_string();
    *state.control_session_id.lock().await = Some(session_id.clone());
    record_mock_bridge_request(&state, "PUT", "/control-session", headers, body).await;
    if state.fail_put_after_write {
        return Err(StatusCode::BAD_GATEWAY);
    }
    Ok(Json(json!({
        "session": { "id": session_id },
        "cdp_endpoint": "http://browser:9222",
    })))
}

async fn mock_delete_control_session(
    headers: HeaderMap,
    State(state): State<MockBridgeState>,
) -> Result<Json<Value>, StatusCode> {
    authorize_mock_bridge(&headers)?;
    *state.control_session_id.lock().await = None;
    record_mock_bridge_request(&state, "DELETE", "/control-session", headers, json!({})).await;
    Ok(Json(json!({ "ok": true })))
}

async fn record_mock_bridge_request(
    state: &MockBridgeState,
    method: &str,
    path: &str,
    headers: HeaderMap,
    body: Value,
) {
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    state.requests.lock().await.push(MockBridgeRequest {
        method: method.to_string(),
        path: path.to_string(),
        authorization,
        body,
    });
}

fn authorize_mock_bridge(headers: &HeaderMap) -> Result<(), StatusCode> {
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    if authorization == Some("Bearer internal-token") {
        Ok(())
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}
