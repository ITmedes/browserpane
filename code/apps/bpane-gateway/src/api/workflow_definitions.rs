use axum::routing::{get, post};

use super::*;

pub(super) fn workflow_definition_routes() -> Router<Arc<ApiState>> {
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
