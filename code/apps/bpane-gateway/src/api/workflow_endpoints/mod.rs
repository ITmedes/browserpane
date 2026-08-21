mod machine;
mod owner;
mod problem;

use super::*;

use problem::WorkflowEndpointApiError;

pub(super) fn workflow_endpoint_routes() -> Router<Arc<ApiState>> {
    owner::routes().merge(machine::routes())
}

async fn authenticate(
    headers: &HeaderMap,
    state: &ApiState,
) -> Result<AuthenticatedPrincipal, WorkflowEndpointApiError> {
    authorize_api_request(headers, &state.auth_validator)
        .await
        .map_err(|error| {
            WorkflowEndpointApiError::new(
                StatusCode::UNAUTHORIZED,
                "authentication_failed",
                "Authentication failed",
                error,
            )
        })
}

async fn load_owner_endpoint(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    project_id: Uuid,
    endpoint_key: &str,
) -> Result<StoredWorkflowEndpoint, WorkflowEndpointApiError> {
    state
        .session_store
        .get_workflow_endpoint_for_owner_project_key(principal, project_id, endpoint_key)
        .await
        .map_err(WorkflowEndpointApiError::from_store)?
        .ok_or_else(|| endpoint_not_found(project_id, endpoint_key))
}

fn endpoint_not_found(project_id: Uuid, endpoint_key: &str) -> WorkflowEndpointApiError {
    WorkflowEndpointApiError::new(
        StatusCode::NOT_FOUND,
        "workflow_endpoint_not_found",
        "Workflow endpoint not found",
        format!("workflow endpoint {endpoint_key} was not found in project {project_id}"),
    )
}

fn owner_principal(endpoint: &StoredWorkflowEndpoint) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        subject: endpoint.owner_subject.clone(),
        issuer: endpoint.owner_issuer.clone(),
        display_name: None,
        client_id: None,
        safe_claims: Default::default(),
    }
}
