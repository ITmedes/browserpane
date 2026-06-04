use axum::routing::{get, post};

use super::*;

pub(super) fn project_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route("/api/v1/projects", post(create_project).get(list_projects))
        .route(
            "/api/v1/projects/{project_id}",
            get(get_project).put(update_project),
        )
        .route(
            "/api/v1/projects/{project_id}/usage",
            get(get_project_usage),
        )
}

async fn list_projects(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ProjectListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let projects = state
        .session_store
        .list_projects_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let projects = project_resources(&state, &principal, projects).await?;
    Ok(Json(ProjectListResponse { projects }))
}

async fn create_project(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertProjectRequest>,
) -> Result<(StatusCode, Json<ProjectResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let project = state
        .session_store
        .create_project(&principal, persist_project_request(request))
        .await
        .map_err(map_session_store_error)?;
    Ok((
        StatusCode::CREATED,
        Json(project_resource(&state, &principal, &project).await?),
    ))
}

async fn get_project(
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ProjectResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let project = load_project(&state, &principal, project_id).await?;
    Ok(Json(project_resource(&state, &principal, &project).await?))
}

async fn update_project(
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertProjectRequest>,
) -> Result<Json<ProjectResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let project = state
        .session_store
        .update_project_for_owner(&principal, project_id, persist_project_request(request))
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("project {project_id} not found"),
                }),
            )
        })?;
    Ok(Json(project_resource(&state, &principal, &project).await?))
}

async fn get_project_usage(
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ProjectUsageResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let project = load_project(&state, &principal, project_id).await?;
    Ok(Json(
        project_resource(&state, &principal, &project).await?.usage,
    ))
}

fn persist_project_request(request: UpsertProjectRequest) -> PersistProjectRequest {
    PersistProjectRequest {
        name: request.name,
        description: request.description,
        labels: request.labels,
        quotas: request.quotas,
        policy: request.policy,
        state: request.state,
    }
}

async fn load_project(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    project_id: Uuid,
) -> Result<StoredProject, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_store
        .get_project_for_owner(principal, project_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("project {project_id} not found"),
                }),
            )
        })
}

async fn project_resources(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    projects: Vec<StoredProject>,
) -> Result<Vec<ProjectResource>, (StatusCode, Json<ErrorResponse>)> {
    let mut resources = Vec::with_capacity(projects.len());
    for project in projects {
        resources.push(project_resource(state, principal, &project).await?);
    }
    Ok(resources)
}

async fn project_resource(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    project: &StoredProject,
) -> Result<ProjectResource, (StatusCode, Json<ErrorResponse>)> {
    let observed_at = Utc::now();
    let active_sessions = state
        .session_store
        .count_active_sessions_for_project(principal, project.id)
        .await
        .map_err(map_session_store_error)?;
    let queued_sessions = state
        .session_store
        .count_queued_sessions_for_project(principal, project.id)
        .await
        .map_err(map_session_store_error)?;
    let session_creations = state
        .session_store
        .count_session_creations_for_project(principal, project.id)
        .await
        .map_err(map_session_store_error)?;
    let active_workflow_runs = state
        .session_store
        .count_active_workflow_runs_for_project(principal, project.id)
        .await
        .map_err(map_session_store_error)?;
    let runtime_usage_ms = state
        .session_store
        .sum_runtime_usage_ms_for_project(principal, project.id, observed_at)
        .await
        .map_err(map_session_store_error)?;
    let (egress_rx_bytes, egress_tx_bytes) = state
        .session_store
        .sum_egress_usage_bytes_for_project(principal, project.id)
        .await
        .map_err(map_session_store_error)?;
    let retained_storage_bytes = state
        .session_store
        .sum_retained_storage_bytes_for_project(principal, project.id)
        .await
        .map_err(map_session_store_error)?;
    Ok(project.to_resource(
        active_sessions,
        queued_sessions,
        session_creations,
        active_workflow_runs,
        runtime_usage_ms,
        egress_rx_bytes,
        egress_tx_bytes,
        retained_storage_bytes,
        observed_at,
    ))
}
