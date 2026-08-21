use axum::routing::{delete, get, post};

use super::*;

pub(super) fn routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints",
            post(create_endpoint).get(list_endpoints),
        )
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}",
            get(get_endpoint).put(update_endpoint),
        )
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/activate",
            post(activate_endpoint),
        )
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/disable",
            post(disable_endpoint),
        )
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/grants",
            get(list_grants).post(upsert_grant),
        )
        .route(
            "/api/v1/projects/{project_id}/workflow-endpoints/{endpoint_key}/grants/{grant_id}",
            delete(delete_grant),
        )
}

async fn create_endpoint(
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertWorkflowEndpointRequest>,
) -> Result<(StatusCode, Json<WorkflowEndpointResource>), WorkflowEndpointApiError> {
    let principal = authenticate(&headers, &state).await?;
    if request.endpoint_key.is_empty() {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_endpoint_key",
            "Invalid endpoint key",
            "endpoint_key is required when creating a workflow endpoint",
        ));
    }
    validate_schema_pair(&request)?;
    let endpoint = state
        .session_store
        .create_workflow_endpoint(&principal, persist_request(project_id, request))
        .await
        .map_err(WorkflowEndpointApiError::from_store)?;
    info!(
        project_id = %project_id,
        endpoint_id = %endpoint.id,
        endpoint_key = %endpoint.endpoint_key,
        "workflow endpoint created"
    );
    Ok((StatusCode::CREATED, Json(endpoint.to_resource())))
}

async fn list_endpoints(
    headers: HeaderMap,
    Path(project_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEndpointListResponse>, WorkflowEndpointApiError> {
    let principal = authenticate(&headers, &state).await?;
    let project = state
        .session_store
        .get_project_for_owner(&principal, project_id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?;
    if project.is_none() {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::NOT_FOUND,
            "project_not_found",
            "Project not found",
            format!("project {project_id} was not found"),
        ));
    }
    let workflow_endpoints = state
        .session_store
        .list_workflow_endpoints_for_owner_project(&principal, project_id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .iter()
        .map(StoredWorkflowEndpoint::to_resource)
        .collect();
    Ok(Json(WorkflowEndpointListResponse { workflow_endpoints }))
}

async fn get_endpoint(
    headers: HeaderMap,
    Path((project_id, endpoint_key)): Path<(Uuid, String)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEndpointResource>, WorkflowEndpointApiError> {
    let principal = authenticate(&headers, &state).await?;
    let endpoint = load_owner_endpoint(&state, &principal, project_id, &endpoint_key).await?;
    Ok(Json(endpoint.to_resource()))
}

async fn update_endpoint(
    headers: HeaderMap,
    Path((project_id, endpoint_key)): Path<(Uuid, String)>,
    State(state): State<Arc<ApiState>>,
    Json(mut request): Json<UpsertWorkflowEndpointRequest>,
) -> Result<Json<WorkflowEndpointResource>, WorkflowEndpointApiError> {
    let principal = authenticate(&headers, &state).await?;
    let endpoint = load_owner_endpoint(&state, &principal, project_id, &endpoint_key).await?;
    if request.endpoint_key != endpoint_key {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::CONFLICT,
            "endpoint_key_immutable",
            "Endpoint key is immutable",
            "endpoint_key must match the endpoint key in the request path",
        ));
    }
    validate_schema_pair(&request)?;
    request.endpoint_key = endpoint_key;
    let endpoint = state
        .session_store
        .update_workflow_endpoint_for_owner(
            &principal,
            endpoint.id,
            persist_request(project_id, request),
        )
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| endpoint_not_found(project_id, &endpoint.endpoint_key))?;
    info!(
        project_id = %project_id,
        endpoint_id = %endpoint.id,
        endpoint_key = %endpoint.endpoint_key,
        "workflow endpoint updated"
    );
    Ok(Json(endpoint.to_resource()))
}

async fn activate_endpoint(
    headers: HeaderMap,
    Path((project_id, endpoint_key)): Path<(Uuid, String)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEndpointResource>, WorkflowEndpointApiError> {
    let principal = authenticate(&headers, &state).await?;
    let endpoint = load_owner_endpoint(&state, &principal, project_id, &endpoint_key).await?;
    validate_activation(&state, &principal, &endpoint).await?;
    let endpoint = state
        .session_store
        .set_workflow_endpoint_state_for_owner(
            &principal,
            endpoint.id,
            WorkflowEndpointState::Active,
        )
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| endpoint_not_found(project_id, &endpoint_key))?;
    info!(
        project_id = %project_id,
        endpoint_id = %endpoint.id,
        endpoint_key = %endpoint.endpoint_key,
        "workflow endpoint activated"
    );
    Ok(Json(endpoint.to_resource()))
}

async fn disable_endpoint(
    headers: HeaderMap,
    Path((project_id, endpoint_key)): Path<(Uuid, String)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEndpointResource>, WorkflowEndpointApiError> {
    let principal = authenticate(&headers, &state).await?;
    let endpoint = load_owner_endpoint(&state, &principal, project_id, &endpoint_key).await?;
    let endpoint = state
        .session_store
        .set_workflow_endpoint_state_for_owner(
            &principal,
            endpoint.id,
            WorkflowEndpointState::Disabled,
        )
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| endpoint_not_found(project_id, &endpoint_key))?;
    info!(
        project_id = %project_id,
        endpoint_id = %endpoint.id,
        endpoint_key = %endpoint.endpoint_key,
        "workflow endpoint disabled"
    );
    Ok(Json(endpoint.to_resource()))
}

async fn list_grants(
    headers: HeaderMap,
    Path((project_id, endpoint_key)): Path<(Uuid, String)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowEndpointGrantListResponse>, WorkflowEndpointApiError> {
    let principal = authenticate(&headers, &state).await?;
    let endpoint = load_owner_endpoint(&state, &principal, project_id, &endpoint_key).await?;
    let grants = state
        .session_store
        .list_workflow_endpoint_grants_for_owner(&principal, &endpoint)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .iter()
        .map(crate::workflow_endpoints::StoredWorkflowEndpointGrant::to_resource)
        .collect();
    Ok(Json(WorkflowEndpointGrantListResponse { grants }))
}

async fn upsert_grant(
    headers: HeaderMap,
    Path((project_id, endpoint_key)): Path<(Uuid, String)>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertWorkflowEndpointGrantRequest>,
) -> Result<Json<WorkflowEndpointGrantResource>, WorkflowEndpointApiError> {
    let principal = authenticate(&headers, &state).await?;
    let endpoint = load_owner_endpoint(&state, &principal, project_id, &endpoint_key).await?;
    let service_principal = state
        .session_store
        .get_service_principal_for_owner(&principal, request.service_principal_id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| {
            WorkflowEndpointApiError::new(
                StatusCode::NOT_FOUND,
                "service_principal_not_found",
                "Service principal not found",
                format!(
                    "service principal {} was not found",
                    request.service_principal_id
                ),
            )
        })?;
    if !service_principal.allowed_project_ids.contains(&project_id) {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::BAD_REQUEST,
            "service_principal_project_mismatch",
            "Service principal is not allowed for this project",
            "the service principal allowed_project_ids must contain the endpoint project",
        ));
    }
    for operation in &request.operations {
        if !service_principal
            .scopes
            .iter()
            .any(|scope| scope == operation.required_scope())
        {
            return Err(WorkflowEndpointApiError::new(
                StatusCode::BAD_REQUEST,
                "service_principal_scope_missing",
                "Service principal scope is missing",
                format!(
                    "service principal does not declare required scope {}",
                    operation.required_scope()
                ),
            ));
        }
    }
    let grant = state
        .session_store
        .upsert_workflow_endpoint_grant_for_owner(
            &principal,
            &endpoint,
            PersistWorkflowEndpointGrantRequest {
                service_principal_id: request.service_principal_id,
                operations: request.operations,
            },
        )
        .await
        .map_err(WorkflowEndpointApiError::from_store)?;
    info!(
        project_id = %project_id,
        endpoint_id = %endpoint.id,
        grant_id = %grant.id,
        service_principal_id = %grant.service_principal_id,
        "workflow endpoint grant upserted"
    );
    Ok(Json(grant.to_resource()))
}

async fn delete_grant(
    headers: HeaderMap,
    Path((project_id, endpoint_key, grant_id)): Path<(Uuid, String, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<StatusCode, WorkflowEndpointApiError> {
    let principal = authenticate(&headers, &state).await?;
    let endpoint = load_owner_endpoint(&state, &principal, project_id, &endpoint_key).await?;
    let deleted = state
        .session_store
        .delete_workflow_endpoint_grant_for_owner(&principal, &endpoint, grant_id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?;
    if !deleted {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::NOT_FOUND,
            "workflow_endpoint_grant_not_found",
            "Workflow endpoint grant not found",
            format!("workflow endpoint grant {grant_id} was not found"),
        ));
    }
    info!(
        project_id = %project_id,
        endpoint_id = %endpoint.id,
        grant_id = %grant_id,
        "workflow endpoint grant revoked"
    );
    Ok(StatusCode::NO_CONTENT)
}

fn persist_request(
    project_id: Uuid,
    request: UpsertWorkflowEndpointRequest,
) -> PersistWorkflowEndpointRequest {
    PersistWorkflowEndpointRequest {
        project_id,
        endpoint_key: request.endpoint_key,
        purpose: request.purpose,
        workflow_definition_id: request.workflow_definition_id,
        workflow_definition_version_id: request.workflow_definition_version_id,
        workflow_version: request.workflow_version,
        input_schema: request.input_schema,
        output_schema: request.output_schema,
        execution_timeout_seconds: request.execution_timeout_seconds,
        inline_result_max_bytes: request.inline_result_max_bytes,
        artifact_behavior: request.artifact_behavior,
        labels: request.labels,
    }
}

fn validate_schema_pair(
    request: &UpsertWorkflowEndpointRequest,
) -> Result<(), WorkflowEndpointApiError> {
    for (name, schema) in [
        ("input_schema", &request.input_schema),
        ("output_schema", &request.output_schema),
    ] {
        if let Err(errors) = crate::workflow_endpoints::validate_endpoint_schema(schema) {
            return Err(WorkflowEndpointApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_json_schema",
                "Invalid JSON Schema",
                format!("{name} must be a valid JSON Schema Draft 2020-12 schema"),
            )
            .with_validation_errors(errors));
        }
    }
    Ok(())
}

async fn validate_activation(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    endpoint: &StoredWorkflowEndpoint,
) -> Result<(), WorkflowEndpointApiError> {
    validate_schema_pair(&UpsertWorkflowEndpointRequest {
        endpoint_key: endpoint.endpoint_key.clone(),
        purpose: endpoint.purpose.clone(),
        workflow_definition_id: endpoint.workflow_definition_id,
        workflow_definition_version_id: endpoint.workflow_definition_version_id,
        workflow_version: endpoint.workflow_version.clone(),
        input_schema: endpoint.input_schema.clone(),
        output_schema: endpoint.output_schema.clone(),
        execution_timeout_seconds: endpoint.execution_timeout_seconds,
        inline_result_max_bytes: endpoint.inline_result_max_bytes,
        artifact_behavior: endpoint.artifact_behavior.clone(),
        labels: endpoint.labels.clone(),
    })?;
    let project = state
        .session_store
        .get_project_for_owner(principal, endpoint.project_id)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| {
            WorkflowEndpointApiError::new(
                StatusCode::NOT_FOUND,
                "project_not_found",
                "Project not found",
                format!("project {} was not found", endpoint.project_id),
            )
        })?;
    if project.state != ProjectState::Active {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::CONFLICT,
            "project_inactive",
            "Project is inactive",
            "an endpoint can only be activated in an active project",
        ));
    }
    let version = state
        .session_store
        .get_workflow_definition_version_for_owner(
            principal,
            endpoint.workflow_definition_id,
            &endpoint.workflow_version,
        )
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .filter(|version| version.id == endpoint.workflow_definition_version_id)
        .ok_or_else(|| {
            WorkflowEndpointApiError::new(
                StatusCode::CONFLICT,
                "workflow_version_binding_invalid",
                "Workflow version binding is invalid",
                "the endpoint must bind an existing immutable workflow definition version",
            )
        })?;
    if version.package.is_none() {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::CONFLICT,
            "workflow_package_required",
            "Approved workflow package required",
            "the bound workflow version predates the supported Phase 0 package contract",
        ));
    }
    crate::workflow::validate_workflow_definition_version_contract(
        &version.executor,
        &version.entrypoint,
        version.source.as_ref(),
        version.input_schema.as_ref(),
        version.output_schema.as_ref(),
        version.package.as_ref(),
    )
    .map_err(|error| {
        WorkflowEndpointApiError::new(
            StatusCode::CONFLICT,
            "workflow_package_invalid",
            "Workflow package is invalid",
            error,
        )
    })?;
    let grants = state
        .session_store
        .list_workflow_endpoint_grants_for_owner(principal, endpoint)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?;
    let mut eligible_invoker = false;
    for grant in grants {
        if !grant
            .operations
            .contains(&WorkflowEndpointGrantOperation::Invoke)
        {
            continue;
        }
        let service_principal = state
            .session_store
            .get_service_principal_for_owner(principal, grant.service_principal_id)
            .await
            .map_err(WorkflowEndpointApiError::from_store)?;
        if service_principal.is_some_and(|service_principal| {
            service_principal.state == ServicePrincipalState::Active
                && service_principal
                    .allowed_project_ids
                    .contains(&endpoint.project_id)
                && service_principal
                    .scopes
                    .iter()
                    .any(|scope| scope == WorkflowEndpointGrantOperation::Invoke.required_scope())
        }) {
            eligible_invoker = true;
            break;
        }
    }
    if !eligible_invoker {
        return Err(WorkflowEndpointApiError::new(
            StatusCode::CONFLICT,
            "eligible_invoker_required",
            "Eligible caller grant required",
            "activate requires an active project-scoped service principal with invoke scope and grant",
        ));
    }
    Ok(())
}
