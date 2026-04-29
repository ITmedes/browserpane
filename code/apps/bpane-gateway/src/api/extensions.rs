use axum::routing::{get, post};

use super::*;

pub(super) fn extension_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/extensions",
            post(create_extension).get(list_extensions),
        )
        .route("/api/v1/extensions/{extension_id}", get(get_extension))
        .route(
            "/api/v1/extensions/{extension_id}/versions",
            post(create_extension_version),
        )
        .route(
            "/api/v1/extensions/{extension_id}/enable",
            post(enable_extension),
        )
        .route(
            "/api/v1/extensions/{extension_id}/disable",
            post(disable_extension),
        )
}

async fn list_extensions(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ExtensionDefinitionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let extensions = state
        .session_store
        .list_extension_definitions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|definition| definition.to_resource())
        .collect();
    Ok(Json(ExtensionDefinitionListResponse { extensions }))
}

async fn create_extension(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateExtensionDefinitionRequest>,
) -> Result<(StatusCode, Json<ExtensionDefinitionResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let extension = state
        .session_store
        .create_extension_definition(
            &principal,
            PersistExtensionDefinitionRequest {
                name: request.name,
                description: request.description,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(extension.to_resource())))
}

async fn get_extension(
    headers: HeaderMap,
    Path(extension_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ExtensionDefinitionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let extension = state
        .session_store
        .get_extension_definition_for_owner(&principal, extension_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("extension {extension_id} not found"),
                }),
            )
        })?;
    Ok(Json(extension.to_resource()))
}

async fn create_extension_version(
    headers: HeaderMap,
    Path(extension_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateExtensionVersionRequest>,
) -> Result<(StatusCode, Json<ExtensionVersionResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let version = state
        .session_store
        .create_extension_version_for_owner(
            &principal,
            PersistExtensionVersionRequest {
                extension_definition_id: extension_id,
                version: request.version,
                install_path: request.install_path,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(version.to_resource())))
}

async fn enable_extension(
    headers: HeaderMap,
    Path(extension_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ExtensionDefinitionResource>, (StatusCode, Json<ErrorResponse>)> {
    set_extension_enabled(headers, extension_id, state, true).await
}

async fn disable_extension(
    headers: HeaderMap,
    Path(extension_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ExtensionDefinitionResource>, (StatusCode, Json<ErrorResponse>)> {
    set_extension_enabled(headers, extension_id, state, false).await
}

async fn set_extension_enabled(
    headers: HeaderMap,
    extension_id: Uuid,
    state: Arc<ApiState>,
    enabled: bool,
) -> Result<Json<ExtensionDefinitionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let extension = state
        .session_store
        .set_extension_definition_enabled_for_owner(&principal, extension_id, enabled)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("extension {extension_id} not found"),
                }),
            )
        })?;
    Ok(Json(extension.to_resource()))
}
