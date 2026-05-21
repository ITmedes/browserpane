use axum::routing::{get, post};

use super::*;

pub(super) fn browser_context_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/browser-contexts",
            post(create_browser_context).get(list_browser_contexts),
        )
        .route(
            "/api/v1/browser-contexts/{context_id}",
            get(get_browser_context).delete(delete_browser_context),
        )
}

async fn list_browser_contexts(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<BrowserContextListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let contexts = state
        .session_store
        .list_browser_contexts_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|context| context.to_resource())
        .collect();
    Ok(Json(BrowserContextListResponse { contexts }))
}

async fn create_browser_context(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateBrowserContextRequest>,
) -> Result<(StatusCode, Json<BrowserContextResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let context = state
        .session_store
        .create_browser_context(
            &principal,
            PersistBrowserContextRequest {
                name: request.name,
                description: request.description,
                labels: request.labels,
                persistence_mode: request.persistence_mode,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(context.to_resource())))
}

async fn get_browser_context(
    headers: HeaderMap,
    Path(context_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<BrowserContextResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let context = state
        .session_store
        .get_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    Ok(Json(context.to_resource()))
}

async fn delete_browser_context(
    headers: HeaderMap,
    Path(context_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<BrowserContextResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let existing = state
        .session_store
        .get_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    if existing.state != BrowserContextState::Deleted {
        state
            .session_manager
            .delete_browser_context_data(context_id)
            .await
            .map_err(|error| {
                (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: error.to_string(),
                    }),
                )
            })?;
    }
    let context = state
        .session_store
        .delete_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    Ok(Json(context.to_resource()))
}
