use axum::routing::{get, post};

use super::super::*;

pub(super) fn workflow_run_produced_file_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/workflow-runs/{run_id}/produced-files",
            post(upload_workflow_run_produced_file).get(list_workflow_run_produced_files),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/produced-files/{file_id}/content",
            get(get_workflow_run_produced_file_content),
        )
}

async fn list_workflow_run_produced_files(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunProducedFileListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    Ok(Json(WorkflowRunProducedFileListResponse {
        files: run
            .produced_files
            .iter()
            .map(|file| file.to_resource(run.id))
            .collect(),
    }))
}

async fn upload_workflow_run_produced_file(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    body: Bytes,
) -> Result<(StatusCode, Json<WorkflowRunProducedFileResource>), (StatusCode, Json<ErrorResponse>)>
{
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let owner = load_session_owner_principal(&state, run.session_id).await?;
    let version = state
        .session_store
        .get_workflow_definition_version_for_owner(
            &owner,
            run.workflow_definition_id,
            &run.workflow_version,
        )
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "workflow definition version {} for workflow {} not found",
                        run.workflow_version, run.workflow_definition_id
                    ),
                }),
            )
        })?;
    let workspace_id = required_header_string(&headers, WORKFLOW_RUN_WORKSPACE_ID_HEADER)?
        .parse::<Uuid>()
        .map_err(|error| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "header {WORKFLOW_RUN_WORKSPACE_ID_HEADER} must be a valid UUID: {error}"
                    ),
                }),
            )
        })?;
    if !version
        .allowed_file_workspace_ids
        .iter()
        .any(|entry| entry == &workspace_id.to_string())
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "workflow definition version {} does not allow file workspace {}",
                    version.version, workspace_id
                ),
            }),
        ));
    }
    let _workspace = state
        .session_store
        .get_file_workspace_for_owner(&owner, workspace_id)
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
        .map_err(|error| {
            state
                .workflow_observability
                .record_produced_file_upload_failure();
            map_workspace_file_store_error(error)
        })?;
    let persisted_file = state
        .session_store
        .create_file_workspace_file_for_owner(
            &owner,
            PersistFileWorkspaceFileRequest {
                id: file_id,
                workspace_id,
                name: file_name.clone(),
                media_type: media_type.clone(),
                byte_count: body.len() as u64,
                sha256_hex: sha256_hex.clone(),
                provenance: provenance.clone(),
                artifact_ref: stored_artifact.artifact_ref.clone(),
            },
        )
        .await;
    let persisted_file = match persisted_file {
        Ok(file) => file,
        Err(error) => {
            state
                .workflow_observability
                .record_produced_file_upload_failure();
            let _ = state
                .workspace_file_store
                .delete(&stored_artifact.artifact_ref)
                .await;
            return Err(map_session_store_error(error));
        }
    };
    let updated_run = state
        .session_store
        .append_workflow_run_produced_file(
            run.id,
            PersistWorkflowRunProducedFileRequest {
                workspace_id,
                file_id,
                file_name,
                media_type,
                byte_count: body.len() as u64,
                sha256_hex,
                provenance,
                artifact_ref: stored_artifact.artifact_ref.clone(),
            },
        )
        .await;
    match updated_run {
        Ok(Some(_)) => {}
        Ok(None) => {
            state
                .workflow_observability
                .record_produced_file_upload_failure();
            let _ = state
                .session_store
                .delete_file_workspace_file_for_owner(&owner, workspace_id, file_id)
                .await;
            let _ = state
                .workspace_file_store
                .delete(&stored_artifact.artifact_ref)
                .await;
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow run {run_id} not found"),
                }),
            ));
        }
        Err(error) => {
            state
                .workflow_observability
                .record_produced_file_upload_failure();
            let _ = state
                .session_store
                .delete_file_workspace_file_for_owner(&owner, workspace_id, file_id)
                .await;
            let _ = state
                .workspace_file_store
                .delete(&stored_artifact.artifact_ref)
                .await;
            return Err(map_session_store_error(error));
        }
    }
    state.workflow_observability.record_produced_file_upload();
    Ok((
        StatusCode::CREATED,
        Json(WorkflowRunProducedFileResource {
            workspace_id,
            file_id,
            file_name: persisted_file.name,
            media_type: persisted_file.media_type,
            byte_count: persisted_file.byte_count,
            sha256_hex: persisted_file.sha256_hex,
            provenance: persisted_file.provenance,
            content_path: format!(
                "/api/v1/workflow-runs/{}/produced-files/{file_id}/content",
                run.id
            ),
            created_at: persisted_file.created_at,
        }),
    ))
}

async fn get_workflow_run_produced_file_content(
    headers: HeaderMap,
    Path((run_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let produced_file = run
        .produced_files
        .iter()
        .find(|file| file.file_id == file_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "workflow run produced file {file_id} was not found for run {run_id}"
                    ),
                }),
            )
        })?;
    let bytes = state
        .workspace_file_store
        .read(&produced_file.artifact_ref)
        .await
        .map_err(|error| match error.io_kind() {
            Some(std::io::ErrorKind::NotFound) => (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: format!("workflow run produced file {file_id} is no longer available"),
                }),
            ),
            _ => map_workspace_file_content_error(error),
        })?;
    let media_type = produced_file
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
                sanitize_content_disposition_filename(&produced_file.file_name)
            ),
            "attachment",
        ),
    );
    Ok(response)
}
