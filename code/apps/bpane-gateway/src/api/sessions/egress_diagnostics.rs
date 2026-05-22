use std::collections::HashSet;
use std::time::Duration as StdDuration;

use chrono::DateTime;
use futures_util::{SinkExt, StreamExt};
use reqwest::Url;
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

use super::super::*;

const DEFAULT_PUBLIC_IP_URL: &str = "https://api.ipify.org?format=json";
const DEFAULT_TLS_PROBE_URL: &str = "https://example.com/";
const DEFAULT_TIMEOUT_MS: u64 = 8_000;
const MIN_TIMEOUT_MS: u64 = 250;
const MAX_TIMEOUT_MS: u64 = 30_000;
const MAX_OBSERVATION_LEN: usize = 160;
const MAX_FAILURE_LEN: usize = 360;

#[derive(Debug, Clone)]
struct EgressProbeOptions {
    public_ip_url: Url,
    tls_probe_url: Url,
    timeout: StdDuration,
}

#[derive(Debug, Clone)]
struct BrowserEgressProbeOutput {
    observed_public_ip: Option<String>,
    observed_tls_issuer: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct CdpTargetResponse {
    id: Option<String>,
    #[serde(rename = "webSocketDebuggerUrl")]
    web_socket_debugger_url: Option<String>,
}

pub(super) async fn run_session_egress_diagnostics_probe(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    payload: Option<Json<RunEgressDiagnosticsProbeRequest>>,
) -> Result<Json<EgressDiagnosticsResource>, (StatusCode, Json<ErrorResponse>)> {
    let session =
        authorize_visible_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let options = probe_options(payload.map(|value| value.0).unwrap_or_default())?;

    let profile_id = session.network_identity.egress_profile_id;
    let observed_at = Utc::now();
    let runtime_assignment = state
        .session_manager
        .describe_session_runtime_assignment_status(session_id)
        .await;
    let probe_result = if !session.state.is_runtime_candidate() {
        failed_probe_result(
            session_id,
            profile_id,
            observed_at,
            format!(
                "session runtime is not active in state {}; connect or start the session before running an active egress probe",
                session.state.as_str()
            ),
        )
    } else if runtime_assignment != Some(SessionRuntimeAssignmentStatus::Ready) {
        failed_probe_result(
            session_id,
            profile_id,
            observed_at,
            format!(
                "session runtime is {}; connect or start the session before running an active egress probe",
                runtime_assignment
                    .map(|status| status.as_str())
                    .unwrap_or("not assigned")
            ),
        )
    } else {
        let runtime = state.session_manager.describe_session_runtime(session_id);
        match runtime.cdp_endpoint.as_deref() {
            Some(endpoint) => match run_browser_egress_probe(endpoint, &options).await {
                Ok(output) => PersistEgressDiagnosticsProbeResult {
                    session_id,
                    profile_id,
                    active_probe_collected: true,
                    observed_public_ip: output.observed_public_ip,
                    observed_tls_issuer: output.observed_tls_issuer,
                    last_failure_reason: None,
                    observed_at,
                },
                Err(error) => failed_probe_result(
                    session_id,
                    profile_id,
                    observed_at,
                    sanitize_failure_reason(&error),
                ),
            },
            None => failed_probe_result(
                session_id,
                profile_id,
                observed_at,
                "session runtime does not expose a CDP endpoint for browser egress probing",
            ),
        }
    };

    state
        .session_store
        .upsert_egress_diagnostics_probe_result(probe_result)
        .await
        .map_err(map_session_store_error)?;

    Ok(Json(
        session_egress_diagnostics(&state, &session)
            .await
            .map_err(map_session_store_error)?,
    ))
}

fn failed_probe_result(
    session_id: Uuid,
    profile_id: Option<Uuid>,
    observed_at: DateTime<Utc>,
    reason: impl Into<String>,
) -> PersistEgressDiagnosticsProbeResult {
    PersistEgressDiagnosticsProbeResult {
        session_id,
        profile_id,
        active_probe_collected: false,
        observed_public_ip: None,
        observed_tls_issuer: None,
        last_failure_reason: Some(reason.into()),
        observed_at,
    }
}

fn probe_options(
    request: RunEgressDiagnosticsProbeRequest,
) -> Result<EgressProbeOptions, (StatusCode, Json<ErrorResponse>)> {
    let public_ip_url = parse_probe_url(
        request
            .public_ip_url
            .as_deref()
            .unwrap_or(DEFAULT_PUBLIC_IP_URL),
        "public_ip_url",
        false,
    )?;
    let tls_probe_url = parse_probe_url(
        request
            .tls_probe_url
            .as_deref()
            .unwrap_or(DEFAULT_TLS_PROBE_URL),
        "tls_probe_url",
        true,
    )?;
    let timeout_ms = request.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS);
    if !(MIN_TIMEOUT_MS..=MAX_TIMEOUT_MS).contains(&timeout_ms) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("timeout_ms must be between {MIN_TIMEOUT_MS} and {MAX_TIMEOUT_MS}"),
            }),
        ));
    }

    Ok(EgressProbeOptions {
        public_ip_url,
        tls_probe_url,
        timeout: StdDuration::from_millis(timeout_ms),
    })
}

fn parse_probe_url(
    value: &str,
    field: &str,
    require_https: bool,
) -> Result<Url, (StatusCode, Json<ErrorResponse>)> {
    let url = Url::parse(value).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("{field} must be an absolute URL: {error}"),
            }),
        )
    })?;
    let valid_scheme = if require_https {
        url.scheme() == "https"
    } else {
        matches!(url.scheme(), "http" | "https")
    };
    if !valid_scheme {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: if require_https {
                    format!("{field} must use https")
                } else {
                    format!("{field} must use http or https")
                },
            }),
        ));
    }
    if value.len() > 2048 {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("{field} must be 2048 characters or less"),
            }),
        ));
    }
    Ok(url)
}

async fn run_browser_egress_probe(
    cdp_endpoint: &str,
    options: &EgressProbeOptions,
) -> Result<BrowserEgressProbeOutput, String> {
    let client = reqwest::Client::builder()
        .timeout(options.timeout)
        .build()
        .map_err(|error| format!("failed to build CDP HTTP client: {error}"))?;
    let endpoint = normalize_cdp_endpoint(cdp_endpoint)?;
    let target = create_cdp_target(&client, &endpoint, options.timeout).await?;
    let target_id = target.id.clone();
    let websocket_url = rewrite_websocket_url(
        target
            .web_socket_debugger_url
            .as_deref()
            .ok_or_else(|| "CDP target did not include webSocketDebuggerUrl".to_string())?,
        &endpoint,
    )?;

    let probe = async {
        let (socket, _) = tokio::time::timeout(
            options.timeout,
            tokio_tungstenite::connect_async(websocket_url.as_str()),
        )
        .await
        .map_err(|_| "CDP WebSocket open timed out".to_string())?
        .map_err(|error| format!("CDP WebSocket open failed: {error}"))?;
        let mut connection = CdpConnection::new(socket);
        connection
            .send("Page.enable", json!({}), options.timeout)
            .await?;
        connection
            .send("Network.enable", json!({}), options.timeout)
            .await?;
        connection
            .send("Runtime.enable", json!({}), options.timeout)
            .await?;
        let body = navigate_and_read_body(
            &mut connection,
            options.public_ip_url.as_str(),
            options.timeout,
        )
        .await?;
        if options.tls_probe_url != options.public_ip_url {
            navigate_and_read_body(
                &mut connection,
                options.tls_probe_url.as_str(),
                options.timeout,
            )
            .await?;
        }

        Ok(BrowserEgressProbeOutput {
            observed_public_ip: extract_public_ip(&body),
            observed_tls_issuer: connection.observed_tls_issuer,
        })
    }
    .await;

    close_cdp_target(&client, &endpoint, target_id.as_deref()).await;
    probe
}

async fn navigate_and_read_body(
    connection: &mut CdpConnection,
    url: &str,
    timeout: StdDuration,
) -> Result<String, String> {
    let navigation = connection
        .send("Page.navigate", json!({ "url": url }), timeout)
        .await?;
    if let Some(error_text) = navigation.get("errorText").and_then(Value::as_str) {
        return Err(format!("CDP navigation failed: {error_text}"));
    }
    connection
        .wait_for_event("Page.loadEventFired", timeout)
        .await?;
    let evaluation = connection
        .send(
            "Runtime.evaluate",
            json!({
                "expression": "(() => document.body?.innerText ?? document.documentElement?.innerText ?? '')()",
                "returnByValue": true,
                "awaitPromise": true
            }),
            timeout,
        )
        .await?;
    if let Some(exception) = evaluation.get("exceptionDetails") {
        return Err(format!(
            "CDP page evaluation failed: {}",
            exception
                .get("text")
                .and_then(Value::as_str)
                .unwrap_or("unknown exception")
        ));
    }
    Ok(evaluation
        .pointer("/result/value")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

fn normalize_cdp_endpoint(value: &str) -> Result<Url, String> {
    let mut url = Url::parse(value).map_err(|error| format!("invalid CDP endpoint: {error}"))?;
    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

async fn create_cdp_target(
    client: &reqwest::Client,
    endpoint: &Url,
    timeout: StdDuration,
) -> Result<CdpTargetResponse, String> {
    let target_url = endpoint
        .join("json/new?about%3Ablank")
        .map_err(|error| format!("failed to build CDP target URL: {error}"))?;
    let response = tokio::time::timeout(timeout, client.put(target_url.clone()).send())
        .await
        .map_err(|_| "CDP target creation timed out".to_string())?
        .map_err(|error| format!("CDP target creation failed: {error}"))?;
    let response = if response.status() == StatusCode::METHOD_NOT_ALLOWED
        || response.status() == StatusCode::NOT_FOUND
    {
        tokio::time::timeout(timeout, client.get(target_url).send())
            .await
            .map_err(|_| "CDP target creation fallback timed out".to_string())?
            .map_err(|error| format!("CDP target creation fallback failed: {error}"))?
    } else {
        response
    };
    if !response.status().is_success() {
        return Err(format!(
            "CDP target creation returned HTTP {}",
            response.status()
        ));
    }
    response
        .json::<CdpTargetResponse>()
        .await
        .map_err(|error| format!("CDP target creation returned invalid JSON: {error}"))
}

async fn close_cdp_target(client: &reqwest::Client, endpoint: &Url, target_id: Option<&str>) {
    let Some(target_id) = target_id else {
        return;
    };
    if let Ok(url) = endpoint.join(&format!("json/close/{target_id}")) {
        let _ = client.get(url).send().await;
    }
}

fn rewrite_websocket_url(raw: &str, endpoint: &Url) -> Result<String, String> {
    let mut url = Url::parse(raw).map_err(|error| format!("invalid CDP WebSocket URL: {error}"))?;
    let scheme = if endpoint.scheme() == "https" {
        "wss"
    } else {
        "ws"
    };
    url.set_scheme(scheme)
        .map_err(|_| "failed to rewrite CDP WebSocket scheme".to_string())?;
    url.set_host(endpoint.host_str())
        .map_err(|_| "failed to rewrite CDP WebSocket host".to_string())?;
    url.set_port(endpoint.port())
        .map_err(|_| "failed to rewrite CDP WebSocket port".to_string())?;
    Ok(url.to_string())
}

struct CdpConnection {
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    next_id: u64,
    seen_events: HashSet<String>,
    observed_tls_issuer: Option<String>,
}

impl CdpConnection {
    fn new(
        socket: tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
    ) -> Self {
        Self {
            socket,
            next_id: 1,
            seen_events: HashSet::new(),
            observed_tls_issuer: None,
        }
    }

    async fn send(
        &mut self,
        method: &str,
        params: Value,
        timeout: StdDuration,
    ) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        self.socket
            .send(Message::Text(
                json!({
                    "id": id,
                    "method": method,
                    "params": params
                })
                .to_string(),
            ))
            .await
            .map_err(|error| format!("CDP send failed: {error}"))?;
        loop {
            let message = self.next_json(timeout).await?;
            if message.get("id").and_then(Value::as_u64) == Some(id) {
                if let Some(error) = message.get("error") {
                    return Err(format!(
                        "CDP {method} failed: {}",
                        error
                            .get("message")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown error")
                    ));
                }
                return Ok(message.get("result").cloned().unwrap_or_else(|| json!({})));
            }
            self.observe_event(&message);
        }
    }

    async fn wait_for_event(&mut self, method: &str, timeout: StdDuration) -> Result<(), String> {
        if self.seen_events.remove(method) {
            return Ok(());
        }
        loop {
            let message = self.next_json(timeout).await?;
            if message.get("method").and_then(Value::as_str) == Some(method) {
                self.observe_event(&message);
                return Ok(());
            }
            self.observe_event(&message);
        }
    }

    async fn next_json(&mut self, timeout: StdDuration) -> Result<Value, String> {
        loop {
            let message = tokio::time::timeout(timeout, self.socket.next())
                .await
                .map_err(|_| "CDP response timed out".to_string())?
                .ok_or_else(|| "CDP WebSocket closed".to_string())?
                .map_err(|error| format!("CDP read failed: {error}"))?;
            match message {
                Message::Text(text) => {
                    return serde_json::from_str(&text)
                        .map_err(|error| format!("CDP message was invalid JSON: {error}"));
                }
                Message::Binary(bytes) => {
                    return serde_json::from_slice(&bytes)
                        .map_err(|error| format!("CDP binary message was invalid JSON: {error}"));
                }
                Message::Close(_) => return Err("CDP WebSocket closed".to_string()),
                _ => {}
            }
        }
    }

    fn observe_event(&mut self, message: &Value) {
        let Some(method) = message.get("method").and_then(Value::as_str) else {
            return;
        };
        self.seen_events.insert(method.to_string());
        if method != "Network.responseReceived" || self.observed_tls_issuer.is_some() {
            return;
        }
        let Some(response) = message.pointer("/params/response") else {
            return;
        };
        let is_https = response
            .get("url")
            .and_then(Value::as_str)
            .is_some_and(|url| url.starts_with("https://"));
        if !is_https {
            return;
        }
        if let Some(issuer) = response
            .pointer("/securityDetails/issuer")
            .and_then(Value::as_str)
            .and_then(sanitize_observation)
        {
            self.observed_tls_issuer = Some(issuer);
        }
    }
}

fn extract_public_ip(body: &str) -> Option<String> {
    let trimmed = body.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(value) = serde_json::from_str::<Value>(trimmed) {
        for key in ["ip", "origin", "query"] {
            if let Some(value) = value.get(key).and_then(Value::as_str) {
                return sanitize_observation(value);
            }
        }
    }
    sanitize_observation(trimmed.lines().next().unwrap_or_default())
}

fn sanitize_observation(value: &str) -> Option<String> {
    let sanitized = value
        .trim()
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_OBSERVATION_LEN)
        .collect::<String>();
    if sanitized.is_empty() {
        None
    } else {
        Some(sanitized)
    }
}

fn sanitize_failure_reason(message: &str) -> String {
    let sanitized = message
        .split_whitespace()
        .map(|token| {
            let lower = token.to_ascii_lowercase();
            if lower.starts_with("http://")
                || lower.starts_with("https://")
                || lower.starts_with("ws://")
                || lower.starts_with("wss://")
            {
                "[url]"
            } else {
                token
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .filter(|ch| !ch.is_control())
        .take(MAX_FAILURE_LEN)
        .collect::<String>();
    if sanitized.is_empty() {
        "egress probe failed".to_string()
    } else {
        sanitized
    }
}
