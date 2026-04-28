use axum::routing::{get, post};

use super::*;

pub(super) fn file_workspace_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/file-workspaces",
            post(create_file_workspace).get(list_file_workspaces),
        )
        .route(
            "/api/v1/file-workspaces/{workspace_id}",
            get(get_file_workspace),
        )
        .route(
            "/api/v1/file-workspaces/{workspace_id}/files",
            post(upload_file_workspace_file).get(list_file_workspace_files),
        )
        .route(
            "/api/v1/file-workspaces/{workspace_id}/files/{file_id}",
            get(get_file_workspace_file).delete(delete_file_workspace_file),
        )
        .route(
            "/api/v1/file-workspaces/{workspace_id}/files/{file_id}/content",
            get(get_file_workspace_file_content),
        )
}

async fn list_file_workspaces(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workspaces = state
        .session_store
        .list_file_workspaces_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|workspace| workspace.to_resource())
        .collect();
    Ok(Json(FileWorkspaceListResponse { workspaces }))
}

async fn create_file_workspace(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateFileWorkspaceRequest>,
) -> Result<(StatusCode, Json<FileWorkspaceResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workspace = state
        .session_store
        .create_file_workspace(
            &principal,
            PersistFileWorkspaceRequest {
                name: request.name,
                description: request.description,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(workspace.to_resource())))
}

async fn get_file_workspace(
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceResource>, (StatusCode, Json<ErrorResponse>)> {
    let workspace = authorize_file_workspace_request(&headers, &state, workspace_id).await?;
    Ok(Json(workspace.to_resource()))
}

async fn list_file_workspace_files(
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceFileListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let _workspace = state
        .session_store
        .get_file_workspace_for_owner(&principal, workspace_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("file workspace {workspace_id} not found"),
                }),
            )
        })?;
    let files = state
        .session_store
        .list_file_workspace_files_for_owner(&principal, workspace_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|file| file.to_resource())
        .collect();
    Ok(Json(FileWorkspaceFileListResponse { files }))
}

async fn upload_file_workspace_file(
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    body: Bytes,
) -> Result<(StatusCode, Json<FileWorkspaceFileResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let _workspace = state
        .session_store
        .get_file_workspace_for_owner(&principal, workspace_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("file workspace {workspace_id} not found"),
                }),
            )
        })?;
    let file_name = required_header_string(&headers, FILE_WORKSPACE_FILE_NAME_HEADER)?;
    let provenance =
        parse_optional_json_object_header(&headers, FILE_WORKSPACE_FILE_PROVENANCE_HEADER)?;
    let media_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let file_id = Uuid::now_v7();
    let sha256_hex = hex::encode(Sha256::digest(body.as_ref()));
    let stored_artifact = state
        .workspace_file_store
        .write(StoreWorkspaceFileRequest {
            workspace_id,
            file_id,
            file_name: file_name.clone(),
            bytes: body.to_vec(),
        })
        .await
        .map_err(map_workspace_file_store_error)?;
    let persisted = state
        .session_store
        .create_file_workspace_file_for_owner(
            &principal,
            PersistFileWorkspaceFileRequest {
                id: file_id,
                workspace_id,
                name: file_name,
                media_type,
                byte_count: body.len() as u64,
                sha256_hex,
                provenance,
                artifact_ref: stored_artifact.artifact_ref.clone(),
            },
        )
        .await;
    let persisted = match persisted {
        Ok(file) => file,
        Err(error) => {
            let _ = state
                .workspace_file_store
                .delete(&stored_artifact.artifact_ref)
                .await;
            return Err(map_session_store_error(error));
        }
    };
    Ok((StatusCode::CREATED, Json(persisted.to_resource())))
}

async fn get_file_workspace_file(
    headers: HeaderMap,
    Path((workspace_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceFileResource>, (StatusCode, Json<ErrorResponse>)> {
    let file =
        authorize_file_workspace_file_request(&headers, &state, workspace_id, file_id).await?;
    Ok(Json(file.to_resource()))
}

async fn get_file_workspace_file_content(
    headers: HeaderMap,
    Path((workspace_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let file =
        authorize_file_workspace_file_request(&headers, &state, workspace_id, file_id).await?;
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

async fn delete_file_workspace_file(
    headers: HeaderMap,
    Path((workspace_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceFileResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let file = state
        .session_store
        .get_file_workspace_file_for_owner(&principal, workspace_id, file_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "file workspace file {file_id} for workspace {workspace_id} not found"
                    ),
                }),
            )
        })?;
    state
        .workspace_file_store
        .delete(&file.artifact_ref)
        .await
        .or_else(|error| match error {
            WorkspaceFileStoreError::Backend(inner)
                if inner.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(())
            }
            other => Err(other),
        })
        .map_err(map_workspace_file_content_error)?;
    let deleted = state
        .session_store
        .delete_file_workspace_file_for_owner(&principal, workspace_id, file_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "file workspace file {file_id} for workspace {workspace_id} not found"
                    ),
                }),
            )
        })?;
    Ok(Json(deleted.to_resource()))
}

async fn authorize_file_workspace_request(
    headers: &HeaderMap,
    state: &ApiState,
    workspace_id: Uuid,
) -> Result<crate::file_workspace::StoredFileWorkspace, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    state
        .session_store
        .get_file_workspace_for_owner(&principal, workspace_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("file workspace {workspace_id} not found"),
                }),
            )
        })
}

async fn authorize_file_workspace_file_request(
    headers: &HeaderMap,
    state: &ApiState,
    workspace_id: Uuid,
    file_id: Uuid,
) -> Result<crate::file_workspace::StoredFileWorkspaceFile, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    state
        .session_store
        .get_file_workspace_file_for_owner(&principal, workspace_id, file_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "file workspace file {file_id} for workspace {workspace_id} not found"
                    ),
                }),
            )
        })
}
