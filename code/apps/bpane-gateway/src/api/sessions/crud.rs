use super::super::*;
use std::collections::HashMap;

pub(super) async fn create_session(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let request = resolve_session_template_defaults(&state, &principal, request).await?;
    let owner_mode = resolve_owner_mode(&state, request.owner_mode)?;
    let stored = create_owned_session(&state, &principal, request, owner_mode, None).await?;

    Ok((
        StatusCode::CREATED,
        Json(
            session_resource(&state, &stored, None)
                .await
                .map_err(map_session_store_error)?,
        ),
    ))
}

pub(super) async fn list_sessions(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Query(raw_query): Query<HashMap<String, String>>,
) -> Result<Json<SessionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let filters = parse_session_catalog_filters(raw_query)?;
    let sessions = state
        .session_store
        .list_sessions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let filtered = sessions
        .into_iter()
        .filter(|session| stored_session_matches_catalog_filters(session, &filters))
        .collect::<Vec<_>>();
    let mut resources = Vec::with_capacity(filtered.len());
    for session in filtered {
        let resource = session_resource(&state, &session, None)
            .await
            .map_err(map_session_store_error)?;
        if session_resource_matches_catalog_filters(&resource, &filters) {
            resources.push(resource);
        }
    }
    let resources = resources
        .into_iter()
        .skip(filters.offset)
        .take(filters.limit.unwrap_or(usize::MAX))
        .collect();

    Ok(Json(SessionListResponse {
        sessions: resources,
    }))
}

pub(super) async fn get_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let stored = authorize_visible_session_request(&headers, &state, session_id).await?;

    Ok(Json(
        session_resource(&state, &stored, None)
            .await
            .map_err(map_session_store_error)?,
    ))
}

#[derive(Default)]
struct SessionCatalogFilters {
    template_id: Option<String>,
    states: Vec<SessionLifecycleState>,
    runtime_states: Vec<String>,
    labels: HashMap<String, String>,
    integration_context: HashMap<String, String>,
    limit: Option<usize>,
    offset: usize,
}

fn parse_session_catalog_filters(
    query: HashMap<String, String>,
) -> Result<SessionCatalogFilters, (StatusCode, Json<ErrorResponse>)> {
    let mut filters = SessionCatalogFilters::default();
    for (key, value) in query {
        if key == "template_id" {
            filters.template_id = Some(value);
        } else if key == "state" {
            filters.states = parse_session_states(&value)?;
        } else if key == "runtime_state" {
            filters.runtime_states = parse_csv(&value);
        } else if key == "limit" {
            filters.limit = Some(parse_positive_usize("limit", &value)?);
        } else if key == "offset" {
            filters.offset = parse_non_negative_usize("offset", &value)?;
        } else if let Some(label_key) = key.strip_prefix("label.") {
            if label_key.trim().is_empty() {
                return Err(bad_request("label filter key must not be empty"));
            }
            filters.labels.insert(label_key.to_string(), value);
        } else if let Some(context_key) = key.strip_prefix("integration.") {
            if context_key.trim().is_empty() {
                return Err(bad_request("integration filter key must not be empty"));
            }
            filters
                .integration_context
                .insert(context_key.to_string(), value);
        } else {
            return Err(bad_request(&format!(
                "unsupported session query parameter: {key}"
            )));
        }
    }
    Ok(filters)
}

fn parse_session_states(
    value: &str,
) -> Result<Vec<SessionLifecycleState>, (StatusCode, Json<ErrorResponse>)> {
    parse_csv(value)
        .into_iter()
        .map(|entry| {
            entry
                .parse::<SessionLifecycleState>()
                .map_err(|_| bad_request(&format!("unsupported session state filter: {entry}")))
        })
        .collect()
}

fn parse_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn parse_positive_usize(
    name: &str,
    value: &str,
) -> Result<usize, (StatusCode, Json<ErrorResponse>)> {
    let parsed = parse_non_negative_usize(name, value)?;
    if parsed == 0 {
        return Err(bad_request(&format!("{name} must be greater than zero")));
    }
    Ok(parsed)
}

fn parse_non_negative_usize(
    name: &str,
    value: &str,
) -> Result<usize, (StatusCode, Json<ErrorResponse>)> {
    value
        .parse::<usize>()
        .map_err(|_| bad_request(&format!("{name} must be a non-negative integer")))
}

fn bad_request(message: &str) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::BAD_REQUEST,
        Json(ErrorResponse {
            error: message.to_string(),
        }),
    )
}

fn stored_session_matches_catalog_filters(
    session: &StoredSession,
    filters: &SessionCatalogFilters,
) -> bool {
    if filters
        .template_id
        .as_ref()
        .is_some_and(|template_id| session.template_id.as_ref() != Some(template_id))
    {
        return false;
    }
    if !filters.states.is_empty() && !filters.states.contains(&session.state) {
        return false;
    }
    if !filters
        .labels
        .iter()
        .all(|(key, value)| session.labels.get(key) == Some(value))
    {
        return false;
    }
    filters.integration_context.iter().all(|(key, value)| {
        session
            .integration_context
            .as_ref()
            .and_then(Value::as_object)
            .and_then(|object| object.get(key))
            .and_then(Value::as_str)
            == Some(value.as_str())
    })
}

fn session_resource_matches_catalog_filters(
    resource: &SessionResource,
    filters: &SessionCatalogFilters,
) -> bool {
    filters.runtime_states.is_empty()
        || filters
            .runtime_states
            .iter()
            .any(|state| state == session_runtime_state_name(resource.status.runtime_state))
}

fn session_runtime_state_name(state: crate::session_control::SessionRuntimeState) -> &'static str {
    match state {
        crate::session_control::SessionRuntimeState::NotStarted => "not_started",
        crate::session_control::SessionRuntimeState::Starting => "starting",
        crate::session_control::SessionRuntimeState::Running => "running",
        crate::session_control::SessionRuntimeState::Released => "released",
        crate::session_control::SessionRuntimeState::Stopping => "stopping",
        crate::session_control::SessionRuntimeState::Stopped => "stopped",
    }
}
