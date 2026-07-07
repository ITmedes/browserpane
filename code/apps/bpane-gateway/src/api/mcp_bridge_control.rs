use axum::routing::get;
use reqwest::Method;

use super::*;

pub(super) fn mcp_bridge_control_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/api/v1/mcp-bridge/health", get(get_mcp_bridge_health))
        .route(
            "/api/v1/mcp-bridge/control-session",
            get(get_mcp_bridge_control_session)
                .put(set_mcp_bridge_control_session)
                .delete(clear_mcp_bridge_control_session),
        )
}

async fn get_mcp_bridge_health(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;

    proxy_bridge_json(&state, Method::GET, BridgeProxyTarget::Health, None)
        .await
        .map(Json)
}

async fn get_mcp_bridge_control_session(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;

    proxy_bridge_json(&state, Method::GET, BridgeProxyTarget::ControlSession, None)
        .await
        .map(Json)
}

async fn set_mcp_bridge_control_session(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<McpBridgeControlSessionRequest>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    authorize_visible_session_request(&headers, &state, req.session_id).await?;

    proxy_bridge_json(
        &state,
        Method::PUT,
        BridgeProxyTarget::ControlSession,
        Some(serde_json::json!({ "session_id": req.session_id })),
    )
    .await
    .map(Json)
}

async fn clear_mcp_bridge_control_session(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<Value>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;

    let current =
        proxy_bridge_json(&state, Method::GET, BridgeProxyTarget::ControlSession, None).await?;
    if let Some(session_id) = control_session_id(&current)? {
        authorize_visible_session_request(&headers, &state, session_id).await?;
    }

    proxy_bridge_json(
        &state,
        Method::DELETE,
        BridgeProxyTarget::ControlSession,
        None,
    )
    .await
    .map(Json)
}

#[derive(Clone, Copy)]
enum BridgeProxyTarget {
    ControlSession,
    Health,
}

async fn proxy_bridge_json(
    state: &ApiState,
    method: Method,
    target: BridgeProxyTarget,
    body: Option<Value>,
) -> Result<Value, (StatusCode, Json<ErrorResponse>)> {
    let config = state.mcp_bridge_control.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "MCP bridge control proxy is not configured".to_string(),
            }),
        )
    })?;
    let url = match target {
        BridgeProxyTarget::ControlSession => config.control_url.clone(),
        BridgeProxyTarget::Health => health_url_for_control_url(&config.control_url)?,
    };
    let client = reqwest::Client::builder()
        .timeout(config.timeout)
        .build()
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to build MCP bridge client: {error}"),
                }),
            )
        })?;
    let mut request = client.request(method.clone(), &url);
    if let Some(token) = &config.bearer_token {
        request = request.bearer_auth(token);
    }
    if let Some(body) = body {
        request = request.json(&body);
    }

    let response = request.send().await.map_err(|error| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: format!("MCP bridge {method} request failed: {error}"),
            }),
        )
    })?;
    let status = response.status();
    let text = response.text().await.map_err(|error| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("failed to read MCP bridge response: {error}"),
            }),
        )
    })?;
    let value = serde_json::from_str::<Value>(&text).map_err(|error| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("MCP bridge returned non-JSON response: {error}"),
            }),
        )
    })?;

    if !status.is_success() {
        return Err((
            StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY),
            Json(ErrorResponse {
                error: bridge_error_message(method, status.as_u16(), value),
            }),
        ));
    }

    Ok(value)
}

fn health_url_for_control_url(
    control_url: &str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let mut url = reqwest::Url::parse(control_url).map_err(|error| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: format!("invalid MCP bridge control URL: {error}"),
            }),
        )
    })?;
    let control_path = url.path().trim_end_matches('/');
    let prefix = control_path
        .rfind('/')
        .map(|index| &control_path[..index + 1])
        .unwrap_or("/");
    url.set_path(&format!("{prefix}health"));
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.to_string())
}

fn control_session_id(value: &Value) -> Result<Option<Uuid>, (StatusCode, Json<ErrorResponse>)> {
    let Some(session) = value.get("session") else {
        return Ok(None);
    };
    if session.is_null() {
        return Ok(None);
    }
    let id = session.get("id").and_then(Value::as_str).ok_or_else(|| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "MCP bridge control response omitted session.id".to_string(),
            }),
        )
    })?;
    Uuid::parse_str(id).map(Some).map_err(|error| {
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: format!("MCP bridge control response returned invalid session id: {error}"),
            }),
        )
    })
}

fn bridge_error_message(method: Method, status: u16, value: Value) -> String {
    let detail = value
        .get("error")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| value.to_string());
    format!("MCP bridge {method} request failed with HTTP {status}: {detail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_url_is_derived_beside_control_session_path() {
        assert_eq!(
            health_url_for_control_url("http://localhost:8931/control-session")
                .unwrap_or_else(|_| panic!("control URL should be valid")),
            "http://localhost:8931/health"
        );
        assert_eq!(
            health_url_for_control_url("https://example.test/mcp/control-session?x=1")
                .unwrap_or_else(|_| panic!("control URL should be valid")),
            "https://example.test/mcp/health"
        );
    }

    #[test]
    fn control_session_id_accepts_null_session() {
        assert!(matches!(
            control_session_id(&serde_json::json!({ "session": null })),
            Ok(None)
        ));
    }

    #[test]
    fn control_session_id_rejects_invalid_session_id() {
        assert!(control_session_id(&serde_json::json!({ "session": { "id": "bad" } })).is_err());
    }
}
