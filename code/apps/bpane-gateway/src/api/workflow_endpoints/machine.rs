use axum::routing::{get, post};
use chrono::Duration as ChronoDuration;

use super::*;

pub(super) fn routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/invocations",
            post(invoke_endpoint),
        )
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/invocations/{invocation_id}",
            get(get_invocation),
        )
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/invocations/{invocation_id}/cancel",
            post(cancel_invocation),
        )
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/invocations/{invocation_id}/artifacts/{file_id}/content",
            get(get_artifact_content),
        )
}

async fn invoke_endpoint(
    headers: HeaderMap,
    Path((project_id, endpoint_key)): Path<(Uuid, String)>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<InvokeWorkflowEndpointRequest>,
) -> Result<(StatusCode, Json<WorkflowEndpointInvocationResource>), WorkflowEndpointApiError> {
    let caller = authenticate(&headers, &state).await?;
    let (endpoint, service_principal) = authorize_machine_operation(
        &state,
        &caller,
        project_id,
        &endpoint_key,
        WorkflowEndpointGrantOperation::Invoke,
        true,
    )
    .await?;
    let idempotency_key = required_idempotency_key(&headers)?;
    if let Err(errors) =
        crate::workflow_endpoints::validate_schema_instance(&endpoint.input_schema, &request.input)
    {
        state
            .workflow_observability
            .record_endpoint_validation_denial();
        info!(
            project_id = %project_id,
            endpoint_id = %endpoint.id,
            service_principal_id = %service_principal.id,
            "workflow endpoint invocation denied by input validation"
        );
        return Err(WorkflowEndpointApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "input_schema_validation_failed",
            "Input schema validation failed",
            "input did not satisfy the active workflow endpoint schema",
        )
        .with_validation_errors(errors));
    }
    let request_fingerprint =
        crate::workflow_endpoints::canonical_request_fingerprint(&serde_json::json!({
            "input": request.input,
            "source_system": request.source_system,
            "source_reference": request.source_reference,
        }))
        .map_err(|_| {
            WorkflowEndpointApiError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "request_fingerprint_failed",
                "Request fingerprint failed",
                "the normalized invocation request could not be fingerprinted",
            )
        })?;
    let reservation = state
        .session_store
        .reserve_workflow_endpoint_invocation(ReserveWorkflowEndpointInvocationRequest {
            endpoint_id: endpoint.id,
            caller_service_principal_id: service_principal.id,
            idempotency_key: idempotency_key.clone(),
            request_fingerprint: request_fingerprint.clone(),
        })
        .await
        .map_err(|error| match error {
            SessionStoreError::Conflict(_) => {
                state
                    .workflow_observability
                    .record_endpoint_invocation_conflict();
                WorkflowEndpointApiError::new(
                    StatusCode::CONFLICT,
                    "idempotency_key_conflict",
                    "Idempotency key conflict",
                    "the idempotency key is already bound to a different normalized request",
                )
            }
            other => WorkflowEndpointApiError::from_store(other),
        })?;
    if !reservation.created {
        state
            .workflow_observability
            .record_endpoint_invocation_replay();
        info!(
            project_id = %project_id,
            endpoint_id = %endpoint.id,
            invocation_id = %reservation.invocation.id,
            service_principal_id = %service_principal.id,
            "workflow endpoint invocation replayed"
        );
        let resource = build_invocation_resource(&state, &endpoint, reservation.invocation).await?;
        return Ok((StatusCode::OK, Json(resource)));
    }

    let invocation = reservation.invocation;
    let execution_deadline_at =
        Utc::now() + ChronoDuration::seconds(i64::from(endpoint.execution_timeout_seconds));
    let owner = owner_principal(&endpoint);
    let create_request = CreateWorkflowRunRequest {
        workflow_id: endpoint.workflow_definition_id,
        version: endpoint.workflow_version.clone(),
        project_id: Some(project_id),
        session: None,
        input: Some(request.input),
        source_system: request.source_system,
        source_reference: request.source_reference,
        client_request_id: Some(format!("workflow-endpoint-invocation:{}", invocation.id)),
        credential_binding_ids: Vec::new(),
        workspace_inputs: Vec::new(),
        labels: Default::default(),
    };
    let endpoint_context = WorkflowEndpointRunContext {
        endpoint_id: endpoint.id,
        invocation_id: invocation.id,
        endpoint_key: endpoint.endpoint_key.clone(),
        caller_service_principal_id: service_principal.id,
        idempotency_key,
        request_fingerprint,
        execution_deadline_at,
    };
    let (_, run) = match workflows::create_workflow_run_for_principal(
        &state,
        &owner,
        create_request,
        Some(endpoint_context),
    )
    .await
    {
        Ok(result) => result,
        Err(error) => {
            state
                .workflow_observability
                .record_endpoint_admission_failure();
            let _ = state
                .session_store
                .fail_workflow_endpoint_invocation(
                    invocation.id,
                    WorkflowRunOutcome {
                        category: WorkflowOutcomeCategory::PermanentTechnicalFailure,
                        code: "invocation_admission_failed".to_string(),
                        message: "workflow invocation could not be admitted".to_string(),
                        details: None,
                        retryable: false,
                        retry_after_seconds: None,
                        caused_by: Some("workflow_endpoint".to_string()),
                    },
                    WorkflowSideEffectState::None,
                )
                .await;
            return Err(WorkflowEndpointApiError::from_legacy(error));
        }
    };
    let invocation = state
        .session_store
        .link_workflow_endpoint_invocation_run(invocation.id, run.id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| {
            WorkflowEndpointApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "invocation_link_failed",
                "Invocation link failed",
                "the workflow run was created but its invocation link could not be persisted",
            )
        })?;
    info!(
        project_id = %project_id,
        endpoint_id = %endpoint.id,
        invocation_id = %invocation.id,
        run_id = %run.id,
        service_principal_id = %service_principal.id,
        "workflow endpoint invocation accepted"
    );
    state
        .workflow_observability
        .record_endpoint_invocation_accepted();
    let resource = build_invocation_resource(&state, &endpoint, invocation).await?;
    Ok((StatusCode::ACCEPTED, Json(resource)))
}

async fn get_invocation(
    headers: HeaderMap,
    Path((project_id, endpoint_key, invocation_id)): Path<(Uuid, String, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEndpointInvocationResource>, WorkflowEndpointApiError> {
    let caller = authenticate(&headers, &state).await?;
    let (endpoint, service_principal) = authorize_machine_operation(
        &state,
        &caller,
        project_id,
        &endpoint_key,
        WorkflowEndpointGrantOperation::Read,
        false,
    )
    .await?;
    let invocation =
        load_caller_invocation(&state, &endpoint, service_principal.id, invocation_id).await?;
    Ok(Json(
        build_invocation_resource(&state, &endpoint, invocation).await?,
    ))
}

async fn cancel_invocation(
    headers: HeaderMap,
    Path((project_id, endpoint_key, invocation_id)): Path<(Uuid, String, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEndpointInvocationResource>, WorkflowEndpointApiError> {
    let caller = authenticate(&headers, &state).await?;
    let (endpoint, service_principal) = authorize_machine_operation(
        &state,
        &caller,
        project_id,
        &endpoint_key,
        WorkflowEndpointGrantOperation::Cancel,
        false,
    )
    .await?;
    let invocation =
        load_caller_invocation(&state, &endpoint, service_principal.id, invocation_id).await?;
    if let Some(run_id) = invocation.run_id {
        let run = state
            .session_store
            .get_workflow_run_by_id(run_id)
            .await
            .map_err(WorkflowEndpointApiError::from_store)?
            .ok_or_else(|| invocation_not_found(invocation_id))?;
        if !run.state.is_terminal() {
            let owner = owner_principal(&endpoint);
            state
                .session_store
                .cancel_automation_task_for_owner(&owner, run.automation_task_id)
                .await
                .map_err(WorkflowEndpointApiError::from_store)?
                .ok_or_else(|| invocation_not_found(invocation_id))?;
            if let Err(error) = state.workflow_lifecycle.cancel_run(run.id).await {
                warn!(
                    run_id = %run.id,
                    invocation_id = %invocation.id,
                    "failed to stop workflow endpoint worker after cancellation: {error}"
                );
            }
            if let Some(updated) = state
                .session_store
                .get_workflow_run_by_id(run.id)
                .await
                .map_err(WorkflowEndpointApiError::from_store)?
            {
                if let (Some(outcome), Some(side_effect_state)) =
                    (updated.outcome.as_ref(), updated.side_effect_state)
                {
                    state
                        .workflow_observability
                        .record_endpoint_terminal(outcome.category, side_effect_state);
                }
            }
        }
    }
    info!(
        project_id = %project_id,
        endpoint_id = %endpoint.id,
        invocation_id = %invocation.id,
        service_principal_id = %service_principal.id,
        "workflow endpoint invocation cancellation requested"
    );
    state
        .workflow_observability
        .record_endpoint_cancel_requested();
    Ok(Json(
        build_invocation_resource(&state, &endpoint, invocation).await?,
    ))
}

async fn get_artifact_content(
    headers: HeaderMap,
    Path((project_id, endpoint_key, invocation_id, file_id)): Path<(Uuid, String, Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, WorkflowEndpointApiError> {
    let caller = authenticate(&headers, &state).await?;
    let (endpoint, service_principal) = authorize_machine_operation(
        &state,
        &caller,
        project_id,
        &endpoint_key,
        WorkflowEndpointGrantOperation::ArtifactRead,
        false,
    )
    .await?;
    let invocation =
        load_caller_invocation(&state, &endpoint, service_principal.id, invocation_id).await?;
    let run_id = invocation
        .run_id
        .ok_or_else(|| invocation_not_found(invocation_id))?;
    let run = state
        .session_store
        .get_workflow_run_by_id(run_id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| invocation_not_found(invocation_id))?;
    let produced_file = run
        .produced_files
        .iter()
        .find(|file| file.file_id == file_id)
        .ok_or_else(|| artifact_not_found(file_id))?;
    if artifact_expired(&endpoint, &run) {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::GONE,
            "artifact_expired",
            "Artifact expired",
            format!("workflow endpoint artifact {file_id} has expired"),
        ));
    }
    let bytes = state
        .workspace_file_store
        .read(&produced_file.artifact_ref)
        .await
        .map_err(|error| match error.io_kind() {
            Some(std::io::ErrorKind::NotFound) => WorkflowEndpointApiError::new(
                StatusCode::GONE,
                "artifact_unavailable",
                "Artifact unavailable",
                format!("workflow endpoint artifact {file_id} is no longer available"),
            ),
            _ => WorkflowEndpointApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "artifact_store_unavailable",
                "Artifact store unavailable",
                "the workflow artifact could not be read",
            ),
        })?;
    let media_type = produced_file
        .media_type
        .as_deref()
        .unwrap_or("application/octet-stream");
    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response.headers_mut().insert(
        CONTENT_TYPE,
        header_value_or_default(media_type, "application/octet-stream"),
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

async fn authorize_machine_operation(
    state: &ApiState,
    caller: &AuthenticatedPrincipal,
    project_id: Uuid,
    endpoint_key: &str,
    operation: WorkflowEndpointGrantOperation,
    require_active_endpoint: bool,
) -> Result<(StoredWorkflowEndpoint, StoredServicePrincipal), WorkflowEndpointApiError> {
    let endpoint = state
        .session_store
        .get_workflow_endpoint_by_project_key(project_id, endpoint_key)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| endpoint_not_found(project_id, endpoint_key))?;
    if require_active_endpoint && endpoint.state != WorkflowEndpointState::Active {
        state
            .workflow_observability
            .record_endpoint_authorization_denial(operation.as_str());
        return Err(WorkflowEndpointApiError::new(
            StatusCode::CONFLICT,
            "workflow_endpoint_inactive",
            "Workflow endpoint is inactive",
            "new invocations require an active workflow endpoint",
        ));
    }
    let client_id = caller.client_id.as_deref().ok_or_else(|| {
        state
            .workflow_observability
            .record_endpoint_authorization_denial(operation.as_str());
        WorkflowEndpointApiError::new(
            StatusCode::FORBIDDEN,
            "confidential_client_required",
            "Confidential client required",
            "workflow endpoint machine routes require an OIDC client-credentials principal",
        )
    })?;
    let owner = owner_principal(&endpoint);
    let service_principal = state
        .session_store
        .get_service_principal_for_owner_by_external_identity(&owner, &caller.issuer, client_id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?;
    let Some(service_principal) = service_principal else {
        state
            .workflow_observability
            .record_endpoint_authorization_denial(operation.as_str());
        return Err(authorization_denied());
    };
    let grant = state
        .session_store
        .get_workflow_endpoint_grant(endpoint.id, service_principal.id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?;
    let allowed = service_principal.state == ServicePrincipalState::Active
        && service_principal.allowed_project_ids.contains(&project_id)
        && service_principal
            .scopes
            .iter()
            .any(|scope| scope == operation.required_scope())
        && grant.is_some_and(|grant| grant.operations.contains(&operation));
    if !allowed {
        state
            .workflow_observability
            .record_endpoint_authorization_denial(operation.as_str());
        info!(
            project_id = %project_id,
            endpoint_id = %endpoint.id,
            service_principal_id = %service_principal.id,
            operation = operation.as_str(),
            "workflow endpoint authorization denied"
        );
        return Err(authorization_denied());
    }
    Ok((endpoint, service_principal))
}

async fn load_caller_invocation(
    state: &ApiState,
    endpoint: &StoredWorkflowEndpoint,
    caller_service_principal_id: Uuid,
    invocation_id: Uuid,
) -> Result<StoredWorkflowEndpointInvocation, WorkflowEndpointApiError> {
    let invocation = state
        .session_store
        .get_workflow_endpoint_invocation(endpoint.id, invocation_id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| invocation_not_found(invocation_id))?;
    if invocation.caller_service_principal_id != caller_service_principal_id {
        return Err(authorization_denied());
    }
    Ok(invocation)
}

async fn build_invocation_resource(
    state: &ApiState,
    endpoint: &StoredWorkflowEndpoint,
    invocation: StoredWorkflowEndpointInvocation,
) -> Result<WorkflowEndpointInvocationResource, WorkflowEndpointApiError> {
    let run = if let Some(run_id) = invocation.run_id {
        let run = state
            .session_store
            .get_workflow_run_by_id(run_id)
            .await
            .map_err(WorkflowEndpointApiError::from_store)?
            .ok_or_else(|| invocation_not_found(invocation.id))?;
        if run.endpoint_id != Some(endpoint.id)
            || run.endpoint_invocation_id != Some(invocation.id)
            || run.caller_service_principal_id != Some(invocation.caller_service_principal_id)
            || run.endpoint_idempotency_key.as_deref() != Some(&invocation.idempotency_key)
            || run.endpoint_request_fingerprint.as_deref() != Some(&invocation.request_fingerprint)
        {
            return Err(WorkflowEndpointApiError::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "invocation_correlation_invalid",
                "Invocation correlation is invalid",
                "the invocation and workflow run correlation evidence did not match",
            ));
        }
        Some(reconcile_endpoint_run(state, endpoint, run).await?)
    } else {
        None
    };
    let base_path = format!(
        "/api/v1/projects/{}/workflow-endpoints/{}/invocations/{}",
        endpoint.project_id, endpoint.endpoint_key, invocation.id
    );
    let artifacts = run
        .as_ref()
        .map(|run| {
            run.produced_files
                .iter()
                .map(|file| WorkflowEndpointArtifactResource {
                    file_id: file.file_id,
                    file_name: file.file_name.clone(),
                    media_type: file
                        .media_type
                        .clone()
                        .unwrap_or_else(|| "application/octet-stream".to_string()),
                    byte_count: file.byte_count,
                    sha256_hex: file.sha256_hex.clone(),
                    provenance: file.provenance.clone(),
                    retention_seconds: endpoint.artifact_behavior.retention_seconds,
                    expires_at: run.completed_at.map(|completed_at| {
                        completed_at
                            + ChronoDuration::seconds(i64::from(
                                endpoint.artifact_behavior.retention_seconds,
                            ))
                    }),
                    content_path: format!("{base_path}/artifacts/{}/content", file.file_id),
                })
                .collect()
        })
        .unwrap_or_default();
    let state_name = run
        .as_ref()
        .map(|run| run.state.as_str())
        .unwrap_or_else(|| {
            if invocation.outcome.is_some() {
                "failed"
            } else {
                "accepted"
            }
        })
        .to_string();
    Ok(WorkflowEndpointInvocationResource {
        id: invocation.id,
        project_id: endpoint.project_id,
        endpoint_key: endpoint.endpoint_key.clone(),
        run_id: invocation.run_id,
        state: state_name,
        source_system: run.as_ref().and_then(|run| run.source_system.clone()),
        source_reference: run.as_ref().and_then(|run| run.source_reference.clone()),
        execution_deadline_at: run.as_ref().and_then(|run| run.execution_deadline_at),
        started_at: run.as_ref().and_then(|run| run.started_at),
        completed_at: run.as_ref().and_then(|run| run.completed_at),
        outcome: run
            .as_ref()
            .and_then(|run| run.outcome.clone())
            .or(invocation.outcome),
        side_effect_state: run
            .as_ref()
            .and_then(|run| run.side_effect_state)
            .unwrap_or(invocation.side_effect_state),
        result: run.as_ref().and_then(|run| {
            (run.state == WorkflowRunState::Succeeded)
                .then(|| run.output.clone())
                .flatten()
        }),
        artifacts,
        links: WorkflowEndpointInvocationLinks {
            self_path: base_path.clone(),
            cancel_path: format!("{base_path}/cancel"),
        },
        created_at: invocation.created_at,
        updated_at: run
            .as_ref()
            .map(|run| run.updated_at)
            .unwrap_or(invocation.updated_at),
    })
}

async fn reconcile_endpoint_run(
    state: &ApiState,
    endpoint: &StoredWorkflowEndpoint,
    run: StoredWorkflowRun,
) -> Result<StoredWorkflowRun, WorkflowEndpointApiError> {
    let terminal_request = if run.state == WorkflowRunState::AwaitingInput {
        Some((
            AutomationTaskState::Failed,
            "external_intervention_required",
            "workflow endpoint stopped because external intervention is required",
        ))
    } else if !run.state.is_terminal()
        && run
            .execution_deadline_at
            .is_some_and(|deadline| Utc::now() >= deadline)
    {
        Some((
            AutomationTaskState::TimedOut,
            "workflow_endpoint_execution_timeout",
            "workflow endpoint execution deadline elapsed",
        ))
    } else {
        None
    };
    let Some((terminal_state, error, message)) = terminal_request else {
        return Ok(run);
    };
    let transition_result = state
        .session_store
        .transition_automation_task(
            run.automation_task_id,
            AutomationTaskTransitionRequest {
                state: terminal_state,
                output: None,
                error: Some(error.to_string()),
                artifact_refs: Vec::new(),
                event_type: "workflow_endpoint.terminal_policy".to_string(),
                event_message: message.to_string(),
                event_data: Some(serde_json::json!({
                    "workflow_endpoint": {
                        "side_effect_state": if run.started_at.is_some() { "uncertain" } else { "none" }
                    }
                })),
            },
        )
        .await;
    if let Err(error) = transition_result {
        if matches!(error, SessionStoreError::Conflict(_)) {
            let current = state
                .session_store
                .get_workflow_run_by_id(run.id)
                .await
                .map_err(WorkflowEndpointApiError::from_store)?;
            if let Some(current) = current.filter(|current| current.state.is_terminal()) {
                return Ok(current);
            }
        }
        return Err(WorkflowEndpointApiError::from_store(error));
    }
    if let Err(error) = state.workflow_lifecycle.cancel_run(run.id).await {
        warn!(
            run_id = %run.id,
            endpoint_id = %endpoint.id,
            "failed to stop terminal workflow endpoint worker: {error}"
        );
    }
    let updated = state
        .session_store
        .get_workflow_run_by_id(run.id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| invocation_not_found(run.endpoint_invocation_id.unwrap_or(run.id)))?;
    if let (Some(outcome), Some(side_effect_state)) =
        (updated.outcome.as_ref(), updated.side_effect_state)
    {
        state
            .workflow_observability
            .record_endpoint_terminal(outcome.category, side_effect_state);
    }
    Ok(updated)
}

fn required_idempotency_key(headers: &HeaderMap) -> Result<String, WorkflowEndpointApiError> {
    let value = headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            WorkflowEndpointApiError::new(
                StatusCode::BAD_REQUEST,
                "idempotency_key_required",
                "Idempotency key required",
                "the Idempotency-Key header is required",
            )
        })?;
    if value.is_empty()
        || value.len() > 128
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::BAD_REQUEST,
            "idempotency_key_invalid",
            "Idempotency key is invalid",
            "the Idempotency-Key header must contain 1-128 non-control characters without surrounding whitespace",
        ));
    }
    Ok(value.to_string())
}

fn artifact_expired(endpoint: &StoredWorkflowEndpoint, run: &StoredWorkflowRun) -> bool {
    run.completed_at.is_some_and(|completed_at| {
        Utc::now()
            >= completed_at
                + ChronoDuration::seconds(i64::from(endpoint.artifact_behavior.retention_seconds))
    })
}

fn authorization_denied() -> WorkflowEndpointApiError {
    WorkflowEndpointApiError::new(
        StatusCode::FORBIDDEN,
        "workflow_endpoint_authorization_denied",
        "Workflow endpoint authorization denied",
        "the authenticated principal is not authorized for this endpoint operation",
    )
}

fn invocation_not_found(invocation_id: Uuid) -> WorkflowEndpointApiError {
    WorkflowEndpointApiError::new(
        StatusCode::NOT_FOUND,
        "workflow_endpoint_invocation_not_found",
        "Workflow endpoint invocation not found",
        format!("workflow endpoint invocation {invocation_id} was not found"),
    )
}

fn artifact_not_found(file_id: Uuid) -> WorkflowEndpointApiError {
    WorkflowEndpointApiError::new(
        StatusCode::NOT_FOUND,
        "workflow_endpoint_artifact_not_found",
        "Workflow endpoint artifact not found",
        format!("workflow endpoint artifact {file_id} was not found"),
    )
}
