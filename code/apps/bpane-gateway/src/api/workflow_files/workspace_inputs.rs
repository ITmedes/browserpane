use std::collections::HashSet;
use std::path::{Component, Path as FsPath};

use axum::response::Response;

use super::*;

pub(super) async fn resolve_workflow_run_workspace_inputs(
    state: &Arc<ApiState>,
    principal: &AuthenticatedPrincipal,
    version: &StoredWorkflowDefinitionVersion,
    requests: Vec<CreateWorkflowRunWorkspaceInputRequest>,
) -> Result<Vec<WorkflowRunWorkspaceInput>, (StatusCode, Json<ErrorResponse>)> {
    if requests.is_empty() {
        return Ok(Vec::new());
    }

    let allowed_workspace_ids = version
        .allowed_file_workspace_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    if allowed_workspace_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "workflow definition version {} does not allow file workspace inputs",
                    version.version
                ),
            }),
        ));
    }

    let mut mount_paths = HashSet::new();
    let mut inputs = Vec::with_capacity(requests.len());
    for request in requests {
        if !allowed_workspace_ids.contains(&request.workspace_id.to_string()) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "workflow definition version {} does not allow file workspace {}",
                        version.version, request.workspace_id
                    ),
                }),
            ));
        }

        let file = state
            .session_store
            .get_file_workspace_file_for_owner(principal, request.workspace_id, request.file_id)
            .await
            .map_err(map_session_store_error)?
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!(
                            "file workspace file {} for workspace {} not found",
                            request.file_id, request.workspace_id
                        ),
                    }),
                )
            })?;
        let mount_path = normalize_workflow_run_workspace_input_mount_path(
            request.mount_path.as_deref().unwrap_or(file.name.as_str()),
        )?;
        if !mount_paths.insert(mount_path.clone()) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "workflow run workspace input mount path {mount_path} is duplicated"
                    ),
                }),
            ));
        }

        inputs.push(WorkflowRunWorkspaceInput {
            id: Uuid::now_v7(),
            workspace_id: request.workspace_id,
            file_id: request.file_id,
            file_name: file.name.clone(),
            media_type: file.media_type.clone(),
            byte_count: file.byte_count,
            sha256_hex: file.sha256_hex.clone(),
            provenance: file.provenance.clone(),
            mount_path,
            artifact_ref: file.artifact_ref.clone(),
        });
    }

    Ok(inputs)
}

fn normalize_workflow_run_workspace_input_mount_path(
    mount_path: &str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let trimmed = mount_path.trim();
    if trimmed.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "workflow run workspace input mount_path must not be empty".to_string(),
            }),
        ));
    }
    let path = FsPath::new(trimmed);
    if path.is_absolute() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "workflow run workspace input mount_path must be relative".to_string(),
            }),
        ));
    }

    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => {
                let value = part.to_string_lossy().trim().to_string();
                if value.is_empty() {
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: "workflow run workspace input mount_path contains an empty component"
                                .to_string(),
                        }),
                    ));
                }
                parts.push(value);
            }
            _ => {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "workflow run workspace input mount_path must not contain traversal or non-normal path components"
                            .to_string(),
                    }),
                ));
            }
        }
    }

    if parts.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "workflow run workspace input mount_path must contain a relative file path"
                    .to_string(),
            }),
        ));
    }

    Ok(parts.join("/"))
}

pub(super) async fn get_workflow_run_workspace_input_content(
    headers: HeaderMap,
    Path((run_id, input_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let workspace_input = run
        .workspace_inputs
        .iter()
        .find(|input| input.id == input_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "workflow run workspace input {input_id} was not found for run {run_id}"
                    ),
                }),
            )
        })?;
    let bytes = state
        .workspace_file_store
        .read(&workspace_input.artifact_ref)
        .await
        .map_err(map_workspace_file_content_error)?;
    let media_type = workspace_input
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
                sanitize_content_disposition_filename(&workspace_input.file_name)
            ),
            "attachment",
        ),
    );
    Ok(response)
}
