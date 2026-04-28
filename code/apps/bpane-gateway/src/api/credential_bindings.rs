use axum::routing::{get, post};

use super::*;

pub(super) fn credential_binding_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/credential-bindings",
            post(create_credential_binding).get(list_credential_bindings),
        )
        .route(
            "/api/v1/credential-bindings/{binding_id}",
            get(get_credential_binding),
        )
}

pub(super) fn workflow_run_credential_binding_routes() -> Router<Arc<ApiState>> {
    Router::new().route(
        "/api/v1/workflow-runs/{run_id}/credential-bindings/{binding_id}/resolved",
        get(get_workflow_run_credential_binding_resolved),
    )
}

async fn list_credential_bindings(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<CredentialBindingListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let credential_bindings = state
        .session_store
        .list_credential_bindings_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|binding| binding.to_resource())
        .collect();
    Ok(Json(CredentialBindingListResponse {
        credential_bindings,
    }))
}

async fn create_credential_binding(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateCredentialBindingRequest>,
) -> Result<(StatusCode, Json<CredentialBindingResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let provider = require_credential_provider(&state)?;
    let binding_id = Uuid::now_v7();
    let stored_secret = match (request.secret_payload, request.external_ref.clone()) {
        (Some(secret_payload), external_ref) => provider
            .store_secret(StoreCredentialSecretRequest {
                binding_id,
                external_ref,
                payload: secret_payload,
            })
            .await
            .map_err(map_credential_provider_error)?,
        (None, Some(external_ref)) => {
            crate::credential_provider::StoredCredentialSecret { external_ref }
        }
        (None, None) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "credential binding requires secret_payload or external_ref".to_string(),
                }),
            ));
        }
    };
    let binding = state
        .session_store
        .create_credential_binding(
            &principal,
            PersistCredentialBindingRequest {
                id: binding_id,
                name: request.name,
                provider: request.provider,
                external_ref: stored_secret.external_ref,
                namespace: request.namespace,
                allowed_origins: request.allowed_origins,
                injection_mode: request.injection_mode,
                totp: request.totp,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(binding.to_resource())))
}

async fn get_credential_binding(
    headers: HeaderMap,
    Path(binding_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<CredentialBindingResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let binding = state
        .session_store
        .get_credential_binding_for_owner(&principal, binding_id)
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
    Ok(Json(binding.to_resource()))
}

async fn get_workflow_run_credential_binding_resolved(
    headers: HeaderMap,
    Path((run_id, binding_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ResolvedWorkflowRunCredentialBindingResource>, (StatusCode, Json<ErrorResponse>)> {
    let run = state
        .session_store
        .get_workflow_run_by_id(run_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow run {run_id} not found"),
                }),
            )
        })?;
    let session = state
        .session_store
        .get_session_by_id(run.session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {} not found", run.session_id),
                }),
            )
        })?;
    let claims = validate_automation_access_request(&headers, &state, run.session_id)?;
    if !automation_access_claims_match_session(&claims, &session) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "automation access token is no longer valid for this session".to_string(),
            }),
        ));
    }
    let binding = run
        .credential_bindings
        .iter()
        .find(|binding| binding.id == binding_id)
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "credential binding {binding_id} is not attached to workflow run {run_id}"
                    ),
                }),
            )
        })?;
    let provider = require_credential_provider(&state)?;
    let resolved = provider
        .resolve_secret(&binding.external_ref)
        .await
        .map_err(map_credential_provider_error)?;
    let owner = load_session_owner_principal(&state, run.session_id).await?;
    let _ = state
        .session_store
        .append_workflow_run_event_for_owner(
            &owner,
            run.id,
            PersistWorkflowRunEventRequest {
                event_type: "workflow_run.credential_binding_resolved".to_string(),
                message: format!(
                    "credential binding {} resolved for workflow run",
                    binding.id
                ),
                data: Some(serde_json::json!({
                    "credential_binding_id": binding.id,
                    "provider": binding.provider,
                    "injection_mode": binding.injection_mode,
                })),
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok(Json(ResolvedWorkflowRunCredentialBindingResource {
        binding: binding.to_resource(run.id),
        payload: resolved.payload,
    }))
}
