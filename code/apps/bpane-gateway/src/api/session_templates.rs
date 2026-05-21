use axum::routing::{get, post};

use super::*;

pub(super) fn session_template_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/session-templates",
            post(create_session_template).get(list_session_templates),
        )
        .route(
            "/api/v1/session-templates/{template_id}",
            get(get_session_template).put(update_session_template),
        )
}

async fn list_session_templates(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionTemplateListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let templates = state
        .session_store
        .list_session_templates_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|template| template.to_resource())
        .collect();
    Ok(Json(SessionTemplateListResponse { templates }))
}

async fn create_session_template(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertSessionTemplateRequest>,
) -> Result<(StatusCode, Json<SessionTemplateResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let template = state
        .session_store
        .create_session_template(&principal, persist_template_request(request))
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(template.to_resource())))
}

async fn get_session_template(
    headers: HeaderMap,
    Path(template_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionTemplateResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let template = state
        .session_store
        .get_session_template_for_owner(&principal, template_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session template {template_id} not found"),
                }),
            )
        })?;
    Ok(Json(template.to_resource()))
}

async fn update_session_template(
    headers: HeaderMap,
    Path(template_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertSessionTemplateRequest>,
) -> Result<Json<SessionTemplateResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let template = state
        .session_store
        .update_session_template_for_owner(
            &principal,
            template_id,
            persist_template_request(request),
        )
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session template {template_id} not found"),
                }),
            )
        })?;
    Ok(Json(template.to_resource()))
}

fn persist_template_request(
    request: UpsertSessionTemplateRequest,
) -> PersistSessionTemplateRequest {
    PersistSessionTemplateRequest {
        name: request.name,
        description: request.description,
        labels: request.labels,
        defaults: request.defaults,
    }
}
