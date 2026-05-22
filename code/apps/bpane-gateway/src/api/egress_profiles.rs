use std::time::Duration as StdDuration;

use axum::routing::{get, post};
use reqwest::Url;
use tokio::net::TcpStream;

use super::*;

const DEFAULT_PROFILE_REACHABILITY_TIMEOUT_MS: u64 = 5_000;
const MIN_PROFILE_REACHABILITY_TIMEOUT_MS: u64 = 250;
const MAX_PROFILE_REACHABILITY_TIMEOUT_MS: u64 = 30_000;
const MAX_PROFILE_REACHABILITY_FAILURE_LEN: usize = 360;

pub(super) fn egress_profile_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/egress-profiles",
            post(create_egress_profile).get(list_egress_profiles),
        )
        .route(
            "/api/v1/egress-profiles/{profile_id}",
            get(get_egress_profile).put(update_egress_profile),
        )
        .route(
            "/api/v1/egress-profiles/{profile_id}/diagnostics",
            get(get_egress_profile_diagnostics),
        )
        .route(
            "/api/v1/egress-profiles/{profile_id}/diagnostics/probe",
            post(run_egress_profile_reachability_probe),
        )
}

async fn list_egress_profiles(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<EgressProfileListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let profiles = state
        .session_store
        .list_egress_profiles_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let reachability = state
        .session_store
        .list_egress_profile_reachability_probe_results_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let profiles = profiles
        .into_iter()
        .map(|profile| profile.to_resource_with_reachability(reachability.get(&profile.id)))
        .collect();
    Ok(Json(EgressProfileListResponse { profiles }))
}

async fn create_egress_profile(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateEgressProfileRequest>,
) -> Result<(StatusCode, Json<EgressProfileResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let request = persist_egress_profile_request(request);
    validate_proxy_auth_binding(&state, &principal, &request).await?;
    let profile = state
        .session_store
        .create_egress_profile(&principal, request)
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(profile.to_resource())))
}

async fn get_egress_profile(
    headers: HeaderMap,
    Path(profile_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<EgressProfileResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let profile = state
        .session_store
        .get_egress_profile_for_owner(&principal, profile_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("egress profile {profile_id} not found"),
                }),
            )
        })?;
    Ok(Json(
        profile_resource_with_reachability(&state, &profile)
            .await
            .map_err(map_session_store_error)?,
    ))
}

async fn get_egress_profile_diagnostics(
    headers: HeaderMap,
    Path(profile_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<EgressDiagnosticsResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let profile = state
        .session_store
        .get_egress_profile_for_owner(&principal, profile_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("egress profile {profile_id} not found"),
                }),
            )
        })?;
    Ok(Json(
        profile_diagnostics_with_reachability(&state, &profile)
            .await
            .map_err(map_session_store_error)?,
    ))
}

async fn update_egress_profile(
    headers: HeaderMap,
    Path(profile_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateEgressProfileRequest>,
) -> Result<Json<EgressProfileResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let request = persist_egress_profile_request(request);
    validate_proxy_auth_binding(&state, &principal, &request).await?;
    let profile = state
        .session_store
        .update_egress_profile_for_owner(&principal, profile_id, request)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("egress profile {profile_id} not found"),
                }),
            )
        })?;
    Ok(Json(
        profile_resource_with_reachability(&state, &profile)
            .await
            .map_err(map_session_store_error)?,
    ))
}

async fn run_egress_profile_reachability_probe(
    headers: HeaderMap,
    Path(profile_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    payload: Option<Json<RunEgressProfileReachabilityProbeRequest>>,
) -> Result<Json<EgressDiagnosticsResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let profile = state
        .session_store
        .get_egress_profile_for_owner(&principal, profile_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("egress profile {profile_id} not found"),
                }),
            )
        })?;
    let timeout = profile_reachability_timeout(payload.map(|value| value.0).unwrap_or_default())?;
    let result = run_profile_reachability(&profile, timeout).await;
    let stored = state
        .session_store
        .upsert_egress_profile_reachability_probe_result(result)
        .await
        .map_err(map_session_store_error)?;
    Ok(Json(
        profile
            .to_diagnostics(None, None, Utc::now())
            .with_profile_reachability_result(Some(&stored)),
    ))
}

fn persist_egress_profile_request(
    request: CreateEgressProfileRequest,
) -> PersistEgressProfileRequest {
    PersistEgressProfileRequest {
        name: request.name,
        description: request.description,
        labels: request.labels,
        proxy: request.proxy,
        bypass_rules: request.bypass_rules,
        custom_ca: request.custom_ca,
        traffic_observation: request.traffic_observation,
        state: request.state,
    }
}

async fn validate_proxy_auth_binding(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    request: &PersistEgressProfileRequest,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let Some(binding_id) = request
        .proxy
        .as_ref()
        .and_then(|proxy| proxy.credential_binding_id)
    else {
        return Ok(());
    };
    if state.credential_provider.is_none() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "egress profile proxy.credential_binding_id requires a configured credential provider".to_string(),
            }),
        ));
    }
    let binding = state
        .session_store
        .get_credential_binding_for_owner(principal, binding_id)
        .await
        .map_err(map_session_store_error)?;
    if binding.is_none() {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("credential binding {binding_id} not found"),
            }),
        ));
    }
    Ok(())
}

async fn profile_resource_with_reachability(
    state: &ApiState,
    profile: &StoredEgressProfile,
) -> Result<EgressProfileResource, SessionStoreError> {
    let reachability = state
        .session_store
        .get_egress_profile_reachability_probe_result(profile.id)
        .await?;
    Ok(profile.to_resource_with_reachability(reachability.as_ref()))
}

async fn profile_diagnostics_with_reachability(
    state: &ApiState,
    profile: &StoredEgressProfile,
) -> Result<EgressDiagnosticsResource, SessionStoreError> {
    let reachability = state
        .session_store
        .get_egress_profile_reachability_probe_result(profile.id)
        .await?;
    Ok(profile
        .to_diagnostics(None, None, Utc::now())
        .with_profile_reachability_result(reachability.as_ref()))
}

fn profile_reachability_timeout(
    request: RunEgressProfileReachabilityProbeRequest,
) -> Result<StdDuration, (StatusCode, Json<ErrorResponse>)> {
    let timeout_ms = request
        .timeout_ms
        .unwrap_or(DEFAULT_PROFILE_REACHABILITY_TIMEOUT_MS);
    if !(MIN_PROFILE_REACHABILITY_TIMEOUT_MS..=MAX_PROFILE_REACHABILITY_TIMEOUT_MS)
        .contains(&timeout_ms)
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "timeout_ms must be between {MIN_PROFILE_REACHABILITY_TIMEOUT_MS} and {MAX_PROFILE_REACHABILITY_TIMEOUT_MS}"
                ),
            }),
        ));
    }
    Ok(StdDuration::from_millis(timeout_ms))
}

async fn run_profile_reachability(
    profile: &StoredEgressProfile,
    timeout: StdDuration,
) -> PersistEgressProfileReachabilityProbeResult {
    let observed_at = Utc::now();
    let result = match profile.proxy.as_ref() {
        Some(proxy) => probe_proxy_authority(&proxy.url, timeout).await,
        None => Ok(()),
    };

    match result {
        Ok(()) => PersistEgressProfileReachabilityProbeResult {
            profile_id: profile.id,
            reachability_collected: true,
            reachability_healthy: true,
            last_failure_reason: None,
            observed_at,
        },
        Err(error) => PersistEgressProfileReachabilityProbeResult {
            profile_id: profile.id,
            reachability_collected: true,
            reachability_healthy: false,
            last_failure_reason: Some(sanitize_profile_reachability_failure(&error)),
            observed_at,
        },
    }
}

async fn probe_proxy_authority(proxy_url: &str, timeout: StdDuration) -> Result<(), String> {
    let url = Url::parse(proxy_url.trim())
        .map_err(|error| format!("proxy url could not be parsed: {error}"))?;
    let host = url
        .host_str()
        .ok_or_else(|| "proxy url did not include a host".to_string())?;
    let port = url.port_or_known_default().ok_or_else(|| {
        "proxy url did not include a port and no default port is known".to_string()
    })?;

    tokio::time::timeout(timeout, TcpStream::connect((host, port)))
        .await
        .map_err(|_| {
            format!(
                "proxy TCP connection timed out after {}ms",
                timeout.as_millis()
            )
        })?
        .map_err(|error| format!("proxy TCP connection failed: {error}"))?;
    Ok(())
}

fn sanitize_profile_reachability_failure(error: &str) -> String {
    let trimmed = error.trim();
    let mut sanitized = String::new();
    for ch in trimmed.chars() {
        if matches!(ch, '\r' | '\n' | '\t') {
            sanitized.push(' ');
        } else {
            sanitized.push(ch);
        }
        if sanitized.len() >= MAX_PROFILE_REACHABILITY_FAILURE_LEN {
            sanitized.truncate(MAX_PROFILE_REACHABILITY_FAILURE_LEN);
            break;
        }
    }
    if sanitized.is_empty() {
        "profile reachability probe failed".to_string()
    } else {
        sanitized
    }
}
