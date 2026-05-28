use std::time::Duration as StdDuration;

use axum::routing::{get, post};
use reqwest::{redirect::Policy as RedirectPolicy, Proxy, Url};
use serde::Deserialize;

use super::*;

const DEFAULT_PROFILE_REACHABILITY_TIMEOUT_MS: u64 = 5_000;
const MIN_PROFILE_REACHABILITY_TIMEOUT_MS: u64 = 250;
const MAX_PROFILE_REACHABILITY_TIMEOUT_MS: u64 = 30_000;
const MAX_PROFILE_REACHABILITY_FAILURE_LEN: usize = 360;
const PROFILE_REACHABILITY_PROBE_URL: &str = "http://example.com/";

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
    let result = run_profile_reachability(&state, &principal, &profile, timeout).await;
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
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    profile: &StoredEgressProfile,
    timeout: StdDuration,
) -> PersistEgressProfileReachabilityProbeResult {
    let observed_at = Utc::now();
    let result = match resolve_profile_proxy_auth(state, principal, profile).await {
        Ok(proxy_auth) => match profile.proxy.as_ref() {
            Some(proxy) => probe_proxy_request(&proxy.url, proxy_auth.as_ref(), timeout).await,
            None => Ok(()),
        },
        Err(error) => Err(error),
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

#[derive(Debug, Clone, Deserialize)]
struct ProfileProxyAuthSecret {
    username: String,
    password: String,
}

async fn resolve_profile_proxy_auth(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    profile: &StoredEgressProfile,
) -> Result<Option<ProfileProxyAuthSecret>, String> {
    let Some(binding_id) = profile
        .proxy
        .as_ref()
        .and_then(|proxy| proxy.credential_binding_id)
    else {
        return Ok(None);
    };
    let Some(provider) = state.credential_provider.as_ref() else {
        return Err("proxy auth credential provider unavailable".to_string());
    };
    let binding = state
        .session_store
        .get_credential_binding_for_owner(principal, binding_id)
        .await
        .map_err(|error| {
            format!("proxy auth credential binding lookup failed for {binding_id}: {error}")
        })?
        .ok_or_else(|| {
            format!("proxy auth credential binding {binding_id} is no longer available")
        })?;
    let secret = provider
        .resolve_secret(&binding.external_ref)
        .await
        .map_err(|error| {
            format!("proxy auth credential binding {binding_id} could not be resolved: {error}")
        })?;
    let payload: ProfileProxyAuthSecret = serde_json::from_value(secret.payload).map_err(|_| {
        "proxy auth credential payload must include username and password strings".to_string()
    })?;
    if payload.username.trim().is_empty() || payload.password.trim().is_empty() {
        return Err(
            "proxy auth credential payload username and password must not be empty".to_string(),
        );
    }
    if payload.username.contains(['\r', '\n']) || payload.password.contains(['\r', '\n']) {
        return Err(
            "proxy auth credential payload username and password must be single-line values"
                .to_string(),
        );
    }
    Ok(Some(payload))
}

async fn probe_proxy_request(
    proxy_url: &str,
    proxy_auth: Option<&ProfileProxyAuthSecret>,
    timeout: StdDuration,
) -> Result<(), String> {
    let url = Url::parse(proxy_url.trim())
        .map_err(|error| format!("proxy url could not be parsed: {error}"))?;
    if url.host_str().is_none() {
        return Err("proxy url did not include a host".to_string());
    }
    url.port_or_known_default().ok_or_else(|| {
        "proxy url did not include a port and no default port is known".to_string()
    })?;

    let mut proxy = Proxy::all(proxy_url.trim())
        .map_err(|error| format!("proxy request configuration failed: {error}"))?;
    if let Some(auth) = proxy_auth {
        proxy = proxy.basic_auth(&auth.username, &auth.password);
    }
    let client = reqwest::Client::builder()
        .timeout(timeout)
        .redirect(RedirectPolicy::none())
        .proxy(proxy)
        .build()
        .map_err(|error| format!("proxy request client could not be built: {error}"))?;
    let response = tokio::time::timeout(timeout, client.get(PROFILE_REACHABILITY_PROBE_URL).send())
        .await
        .map_err(|_| format!("proxy request timed out after {}ms", timeout.as_millis()))?
        .map_err(|error| format!("proxy request failed: {error}"))?;
    if response.status() == reqwest::StatusCode::PROXY_AUTHENTICATION_REQUIRED {
        return Err("proxy authentication was required or credentials were rejected".to_string());
    }
    if !response.status().is_success() {
        let availability_hint = if response.status().is_server_error() {
            "proxy or upstream target was unavailable"
        } else {
            "proxy or upstream target rejected the request"
        };
        return Err(format!(
            "proxy request returned HTTP {}; {availability_hint}",
            response.status().as_u16(),
        ));
    }
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
