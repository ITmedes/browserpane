use super::super::workflow_files::{
    prepare_workflow_run_source_snapshot, resolve_workflow_run_workspace_inputs,
};
use super::super::*;
use sha2::{Digest, Sha256};
use std::collections::HashSet;

async fn resolve_workflow_run_credential_bindings(
    state: &Arc<ApiState>,
    principal: &AuthenticatedPrincipal,
    version: &StoredWorkflowDefinitionVersion,
    requested_ids: Vec<Uuid>,
) -> Result<Vec<WorkflowRunCredentialBinding>, (StatusCode, Json<ErrorResponse>)> {
    if requested_ids.is_empty() {
        return Ok(Vec::new());
    }

    let allowed_binding_ids = version
        .allowed_credential_binding_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    if allowed_binding_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "workflow definition version {} does not allow credential bindings",
                    version.version
                ),
            }),
        ));
    }

    let mut seen_ids = HashSet::new();
    let mut bindings = Vec::with_capacity(requested_ids.len());
    for binding_id in requested_ids {
        if !seen_ids.insert(binding_id) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("workflow run credential binding {binding_id} is duplicated"),
                }),
            ));
        }
        if !allowed_binding_ids.contains(&binding_id.to_string()) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "workflow definition version {} does not allow credential binding {}",
                        version.version, binding_id
                    ),
                }),
            ));
        }
        let binding = state
            .session_store
            .get_credential_binding_for_owner(principal, binding_id)
            .await
            .map_err(map_session_store_error)?
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("credential binding {binding_id} not found"),
                    }),
                )
            })?;
        bindings.push(binding.to_workflow_run_binding());
    }
    Ok(bindings)
}

fn canonicalize_json(value: Value) -> Value {
    match value {
        Value::Object(object) => {
            let mut keys = object.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let mut normalized = serde_json::Map::new();
            for key in keys {
                if let Some(entry) = object.get(&key) {
                    normalized.insert(key, canonicalize_json(entry.clone()));
                }
            }
            Value::Object(normalized)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(canonicalize_json).collect()),
        other => other,
    }
}

fn workflow_run_request_fingerprint(
    request: &CreateWorkflowRunRequest,
) -> Result<Option<String>, (StatusCode, Json<ErrorResponse>)> {
    let Some(client_request_id) = request.client_request_id.as_ref() else {
        return Ok(None);
    };
    if client_request_id.trim().is_empty() {
        return Ok(None);
    }

    let mut credential_binding_ids = request
        .credential_binding_ids
        .iter()
        .map(Uuid::to_string)
        .collect::<Vec<_>>();
    credential_binding_ids.sort();

    let mut workspace_inputs = request
        .workspace_inputs
        .iter()
        .map(|input| {
            serde_json::json!({
                "workspace_id": input.workspace_id,
                "file_id": input.file_id,
                "mount_path": input.mount_path,
            })
        })
        .collect::<Vec<_>>();
    workspace_inputs.sort_by(|left, right| {
        let left_key = (
            left["workspace_id"].as_str().unwrap_or_default(),
            left["file_id"].as_str().unwrap_or_default(),
            left["mount_path"].as_str().unwrap_or_default(),
        );
        let right_key = (
            right["workspace_id"].as_str().unwrap_or_default(),
            right["file_id"].as_str().unwrap_or_default(),
            right["mount_path"].as_str().unwrap_or_default(),
        );
        left_key.cmp(&right_key)
    });

    let descriptor = canonicalize_json(serde_json::json!({
        "workflow_id": request.workflow_id,
        "version": request.version,
        "session": request.session,
        "input": request.input,
        "source_system": request.source_system,
        "source_reference": request.source_reference,
        "credential_binding_ids": credential_binding_ids,
        "workspace_inputs": workspace_inputs,
        "labels": request.labels,
    }));
    let bytes = serde_json::to_vec(&descriptor).map_err(|error| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("failed to encode workflow run request fingerprint: {error}"),
            }),
        )
    })?;
    Ok(Some(hex::encode(Sha256::digest(bytes))))
}

pub(super) async fn create_workflow_run(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateWorkflowRunRequest>,
) -> Result<(StatusCode, Json<WorkflowRunResource>), (StatusCode, Json<ErrorResponse>)> {
    let request_fingerprint = workflow_run_request_fingerprint(&request)?;
    let CreateWorkflowRunRequest {
        workflow_id,
        version: workflow_version_name,
        session,
        input,
        source_system,
        source_reference,
        client_request_id,
        credential_binding_ids,
        workspace_inputs,
        labels,
    } = request;
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    if let Some(client_request_id) = client_request_id
        .as_deref()
        .filter(|value| !value.is_empty())
    {
        if let Some(existing_run) = state
            .session_store
            .find_workflow_run_by_client_request_id_for_owner(&principal, client_request_id)
            .await
            .map_err(map_session_store_error)?
        {
            if existing_run.create_request_fingerprint == request_fingerprint {
                return Ok((
                    StatusCode::OK,
                    Json(build_workflow_run_resource(&state, &existing_run).await?),
                ));
            }
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!(
                        "workflow run client_request_id {} is already bound to a different request",
                        client_request_id
                    ),
                }),
            ));
        }
    }
    let workflow = state
        .session_store
        .get_workflow_definition_for_owner(&principal, workflow_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow definition {workflow_id} not found"),
                }),
            )
        })?;
    let version = state
        .session_store
        .get_workflow_definition_version_for_owner(&principal, workflow_id, &workflow_version_name)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "workflow definition version {} for workflow {} not found",
                        workflow_version_name, workflow_id
                    ),
                }),
            )
        })?;
    let source_snapshot =
        prepare_workflow_run_source_snapshot(&state, &principal, &workflow, &version).await?;
    let credential_bindings = resolve_workflow_run_credential_bindings(
        &state,
        &principal,
        &version,
        credential_binding_ids,
    )
    .await?;
    let workspace_inputs =
        resolve_workflow_run_workspace_inputs(&state, &principal, &version, workspace_inputs)
            .await?;
    let (session, session_source) = resolve_task_session_binding(
        &state,
        &principal,
        session,
        version.default_session.as_ref(),
        Some(&version.allowed_extension_ids),
    )
    .await?;
    let task = state
        .session_store
        .create_automation_task(
            &principal,
            PersistAutomationTaskRequest {
                display_name: Some(format!("{} {}", workflow.name, version.version)),
                executor: version.executor.clone(),
                session_id: session.id,
                session_source,
                input: input.clone(),
                labels: labels.clone(),
            },
        )
        .await
        .map_err(map_session_store_error)?;
    let run = state
        .session_store
        .create_workflow_run(
            &principal,
            PersistWorkflowRunRequest {
                workflow_definition_id: workflow.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                session_id: session.id,
                automation_task_id: task.id,
                source_system,
                source_reference,
                client_request_id,
                create_request_fingerprint: request_fingerprint,
                source_snapshot,
                extensions: session.extensions.clone(),
                credential_bindings,
                workspace_inputs,
                input,
                labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    if run.created {
        if let Err(error) = state
            .workflow_lifecycle
            .ensure_run_started(&version.executor, run.run.id)
            .await
        {
            warn!(
                run_id = %run.run.id,
                workflow_definition_id = %workflow.id,
                workflow_version = %version.version,
                "failed to auto-launch workflow worker: {error}"
            );
        }
    }
    let created = run.created;
    let run = state
        .session_store
        .get_workflow_run_by_id(run.run.id)
        .await
        .map_err(map_session_store_error)?
        .unwrap_or(run.run);
    Ok((
        if created {
            StatusCode::CREATED
        } else {
            StatusCode::OK
        },
        Json(build_workflow_run_resource(&state, &run).await?),
    ))
}
