use axum::routing::{get, post};

use super::*;

pub(super) fn session_file_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/sessions/{session_id}/files",
            get(list_session_files),
        )
        .route(
            "/api/v1/sessions/{session_id}/files/{file_id}",
            get(get_session_file),
        )
        .route(
            "/api/v1/sessions/{session_id}/files/{file_id}/content",
            get(get_session_file_content),
        )
        .route(
            "/api/v1/sessions/{session_id}/file-bindings",
            post(create_session_file_binding).get(list_session_file_bindings),
        )
        .route(
            "/api/v1/sessions/{session_id}/file-bindings/{binding_id}",
            get(get_session_file_binding).delete(remove_session_file_binding),
        )
        .route(
            "/api/v1/sessions/{session_id}/file-bindings/{binding_id}/content",
            get(get_session_file_binding_content),
        )
}

async fn list_session_files(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionFileListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_visible_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let files = state
        .session_store
        .list_session_files_for_session(session_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|file| file.to_resource())
        .collect();

    Ok(Json(SessionFileListResponse { files }))
}

async fn get_session_file(
    headers: HeaderMap,
    Path((session_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionFileResource>, (StatusCode, Json<ErrorResponse>)> {
    let file = authorize_session_file_request(&headers, &state, session_id, file_id).await?;
    Ok(Json(file.to_resource()))
}

async fn get_session_file_content(
    headers: HeaderMap,
    Path((session_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let file = authorize_session_file_request(&headers, &state, session_id, file_id).await?;
    let bytes = state
        .workspace_file_store
        .read(&file.artifact_ref)
        .await
        .map_err(map_workspace_file_content_error)?;
    let media_type = file
        .media_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response.headers_mut().insert(
        CONTENT_TYPE,
        header_value_or_default(&media_type, "application/octet-stream"),
    );
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        header_value_or_default(
            &format!(
                "attachment; filename=\"{}\"",
                sanitize_content_disposition_filename(&file.name)
            ),
            "attachment",
        ),
    );
    Ok(response)
}

async fn create_session_file_binding(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateSessionFileBindingRequest>,
) -> Result<(StatusCode, Json<SessionFileBindingResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let binding = state
        .session_store
        .create_session_file_binding_for_owner(
            &principal,
            PersistSessionFileBindingRequest {
                id: Uuid::now_v7(),
                session_id,
                workspace_id: request.workspace_id,
                file_id: request.file_id,
                mount_path: request.mount_path,
                mode: request.mode,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;

    Ok((StatusCode::CREATED, Json(binding.to_resource())))
}

async fn list_session_file_bindings(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionFileBindingListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_visible_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let bindings = state
        .session_store
        .list_session_file_bindings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|binding| binding.to_resource())
        .collect();

    Ok(Json(SessionFileBindingListResponse { bindings }))
}

async fn get_session_file_binding(
    headers: HeaderMap,
    Path((session_id, binding_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionFileBindingResource>, (StatusCode, Json<ErrorResponse>)> {
    let binding =
        authorize_session_file_binding_request(&headers, &state, session_id, binding_id).await?;
    Ok(Json(binding.to_resource()))
}

async fn get_session_file_binding_content(
    headers: HeaderMap,
    Path((session_id, binding_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let binding =
        authorize_session_file_binding_request(&headers, &state, session_id, binding_id).await?;
    let bytes = state
        .workspace_file_store
        .read(&binding.artifact_ref)
        .await
        .map_err(map_workspace_file_content_error)?;
    let media_type = binding
        .media_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response.headers_mut().insert(
        CONTENT_TYPE,
        header_value_or_default(&media_type, "application/octet-stream"),
    );
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        header_value_or_default(
            &format!(
                "attachment; filename=\"{}\"",
                sanitize_content_disposition_filename(&binding.file_name)
            ),
            "attachment",
        ),
    );
    Ok(response)
}

async fn remove_session_file_binding(
    headers: HeaderMap,
    Path((session_id, binding_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionFileBindingResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let binding = state
        .session_store
        .remove_session_file_binding_for_owner(&principal, session_id, binding_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "session file binding {binding_id} for session {session_id} not found"
                    ),
                }),
            )
        })?;
    Ok(Json(binding.to_resource()))
}

async fn authorize_session_file_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
    file_id: Uuid,
) -> Result<crate::session_files::StoredSessionFile, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_visible_session_request_with_automation_access(headers, state, session_id)
            .await?;
    state
        .session_store
        .get_session_file_for_session(session_id, file_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session file {file_id} for session {session_id} not found"),
                }),
            )
        })
}

async fn authorize_session_file_binding_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
    binding_id: Uuid,
) -> Result<crate::session_files::StoredSessionFileBinding, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_visible_session_request_with_automation_access(headers, state, session_id)
            .await?;
    state
        .session_store
        .get_session_file_binding_for_session(session_id, binding_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "session file binding {binding_id} for session {session_id} not found"
                    ),
                }),
            )
        })
}
