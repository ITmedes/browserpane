use axum::routing::{get, post};

use super::*;

pub(super) fn workflow_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/workflows",
            post(create_workflow_definition).get(list_workflow_definitions),
        )
        .route(
            "/api/v1/workflows/{workflow_id}",
            get(get_workflow_definition),
        )
        .route(
            "/api/v1/workflows/{workflow_id}/versions",
            post(create_workflow_definition_version),
        )
        .route(
            "/api/v1/workflows/{workflow_id}/versions/{version}",
            get(get_workflow_definition_version),
        )
        .route("/api/v1/workflow-runs", post(create_workflow_run))
        .route("/api/v1/workflow-runs/{run_id}", get(get_workflow_run))
        .route(
            "/api/v1/workflow-runs/{run_id}/source-snapshot/content",
            get(get_workflow_run_source_snapshot_content),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/workspace-inputs/{input_id}/content",
            get(get_workflow_run_workspace_input_content),
        )
}

async fn list_workflow_definitions(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowDefinitionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workflows = state
        .session_store
        .list_workflow_definitions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|workflow| workflow.to_resource())
        .collect();
    Ok(Json(WorkflowDefinitionListResponse { workflows }))
}

async fn create_workflow_definition(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateWorkflowDefinitionRequest>,
) -> Result<(StatusCode, Json<WorkflowDefinitionResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workflow = state
        .session_store
        .create_workflow_definition(
            &principal,
            PersistWorkflowDefinitionRequest {
                name: request.name,
                description: request.description,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(workflow.to_resource())))
}

async fn get_workflow_definition(
    headers: HeaderMap,
    Path(workflow_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowDefinitionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
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
    Ok(Json(workflow.to_resource()))
}

async fn create_workflow_definition_version(
    headers: HeaderMap,
    Path(workflow_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateWorkflowDefinitionVersionRequest>,
) -> Result<(StatusCode, Json<WorkflowDefinitionVersionResource>), (StatusCode, Json<ErrorResponse>)>
{
    let CreateWorkflowDefinitionVersionRequest {
        version,
        executor,
        entrypoint,
        source,
        input_schema,
        output_schema,
        default_session,
        allowed_credential_binding_ids,
        allowed_extension_ids,
        allowed_file_workspace_ids,
    } = request;
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let resolved_source = state
        .workflow_source_resolver
        .resolve(source)
        .await
        .map_err(map_workflow_source_error)?;
    validate_workflow_source_entrypoint(resolved_source.as_ref(), &entrypoint)
        .map_err(map_workflow_source_error)?;
    let version = state
        .session_store
        .create_workflow_definition_version(
            &principal,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: workflow_id,
                version,
                executor,
                entrypoint,
                source: resolved_source,
                input_schema,
                output_schema,
                default_session,
                allowed_credential_binding_ids,
                allowed_extension_ids,
                allowed_file_workspace_ids,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(version.to_resource())))
}

async fn get_workflow_definition_version(
    headers: HeaderMap,
    Path((workflow_id, version)): Path<(Uuid, String)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowDefinitionVersionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let version_resource = state
        .session_store
        .get_workflow_definition_version_for_owner(&principal, workflow_id, &version)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "workflow definition version {version} for workflow {workflow_id} not found"
                    ),
                }),
            )
        })?;
    Ok(Json(version_resource.to_resource()))
}

async fn prepare_workflow_run_source_snapshot(
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
) -> Result<crate::file_workspace::StoredFileWorkspaceFile, (StatusCode, Json<ErrorResponse>)> {
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

async fn resolve_workflow_run_workspace_inputs(
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

async fn create_workflow_run(
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

async fn get_workflow_run(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let mut run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    if !run.state.is_terminal() {
        let _ = state.workflow_lifecycle.reconcile_waiting_runs().await;
        if let Some(updated) = state
            .session_store
            .get_workflow_run_by_id(run.id)
            .await
            .map_err(map_session_store_error)?
        {
            run = updated;
        }
    }
    Ok(Json(build_workflow_run_resource(&state, &run).await?))
}

async fn get_workflow_run_source_snapshot_content(
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

async fn get_workflow_run_workspace_input_content(
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
