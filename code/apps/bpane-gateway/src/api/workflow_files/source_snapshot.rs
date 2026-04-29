use std::collections::HashMap;

use axum::response::Response;
use chrono::Utc;
use sha2::{Digest, Sha256};

use super::*;

pub(super) async fn prepare_workflow_run_source_snapshot(
    state: &Arc<ApiState>,
    principal: &AuthenticatedPrincipal,
    workflow: &StoredWorkflowDefinition,
    version: &StoredWorkflowDefinitionVersion,
) -> Result<Option<WorkflowRunSourceSnapshot>, (StatusCode, Json<ErrorResponse>)> {
    let Some(source) = version.source.as_ref() else {
        return Ok(None);
    };
    let archive = state
        .workflow_source_resolver
        .materialize_archive(source, &version.entrypoint)
        .await
        .map_err(map_workflow_source_error)?;
    let archive_source = archive.source.clone();
    let archive_file_name = archive.file_name.clone();
    let archive_media_type = Some(archive.media_type.clone());
    let workspace = state
        .session_store
        .create_file_workspace(
            principal,
            PersistFileWorkspaceRequest {
                name: format!("{} {} source", workflow.name, version.version),
                description: Some(format!(
                    "Immutable source snapshot for workflow {} {}",
                    workflow.name, version.version
                )),
                labels: HashMap::from([
                    ("managed_by".to_string(), "workflow_run".to_string()),
                    (
                        "workflow_definition_id".to_string(),
                        workflow.id.to_string(),
                    ),
                    (
                        "workflow_definition_version_id".to_string(),
                        version.id.to_string(),
                    ),
                    ("workflow_version".to_string(), version.version.clone()),
                ]),
            },
        )
        .await
        .map_err(map_session_store_error)?;
    let file = persist_workflow_source_archive_file(
        state,
        principal,
        workspace.id,
        workflow,
        version,
        archive,
    )
    .await?;
    Ok(Some(WorkflowRunSourceSnapshot {
        source: archive_source,
        entrypoint: version.entrypoint.clone(),
        workspace_id: workspace.id,
        file_id: file.id,
        file_name: archive_file_name,
        media_type: archive_media_type,
    }))
}

async fn persist_workflow_source_archive_file(
    state: &Arc<ApiState>,
    principal: &AuthenticatedPrincipal,
    workspace_id: Uuid,
    workflow: &StoredWorkflowDefinition,
    version: &StoredWorkflowDefinitionVersion,
    archive: WorkflowSourceArchive,
) -> Result<crate::workspaces::StoredFileWorkspaceFile, (StatusCode, Json<ErrorResponse>)> {
    let WorkflowSourceArchive {
        source,
        file_name,
        media_type,
        bytes,
    } = archive;
    let file_id = Uuid::now_v7();
    let byte_count = bytes.len() as u64;
    let sha256_hex = hex::encode(Sha256::digest(bytes.as_slice()));
    let provenance = Some(serde_json::json!({
        "kind": "workflow_source_snapshot",
        "workflow_definition_id": workflow.id,
        "workflow_definition_version_id": version.id,
        "workflow_version": version.version,
        "entrypoint": version.entrypoint,
        "source": source,
        "created_at": Utc::now(),
    }));
    let stored_artifact = state
        .workspace_file_store
        .write(StoreWorkspaceFileRequest {
            workspace_id,
            file_id,
            file_name: file_name.clone(),
            bytes,
        })
        .await
        .map_err(map_workspace_file_store_error)?;
    let persisted = state
        .session_store
        .create_file_workspace_file_for_owner(
            principal,
            PersistFileWorkspaceFileRequest {
                id: file_id,
                workspace_id,
                name: file_name,
                media_type: Some(media_type),
                byte_count,
                sha256_hex,
                provenance,
                artifact_ref: stored_artifact.artifact_ref.clone(),
            },
        )
        .await;
    match persisted {
        Ok(file) => Ok(file),
        Err(error) => {
            let _ = state
                .workspace_file_store
                .delete(&stored_artifact.artifact_ref)
                .await;
            Err(map_session_store_error(error))
        }
    }
}

pub(super) async fn get_workflow_run_source_snapshot_content(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let source_snapshot = run.source_snapshot.as_ref().ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("workflow run {run_id} does not have a source snapshot"),
            }),
        )
    })?;
    let principal = load_session_owner_principal(&state, run.session_id).await?;
    let file = state
        .session_store
        .get_file_workspace_file_for_owner(
            &principal,
            source_snapshot.workspace_id,
            source_snapshot.file_id,
        )
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "workflow run source snapshot file {} for workspace {} was not found",
                        source_snapshot.file_id, source_snapshot.workspace_id
                    ),
                }),
            )
        })?;
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
