use axum::routing::{get, post};

use super::*;

pub(super) fn identity_mapping_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/identity-mappings",
            post(create_identity_mapping).get(list_identity_mappings),
        )
        .route(
            "/api/v1/identity-mappings/{identity_mapping_id}",
            get(get_identity_mapping).put(update_identity_mapping),
        )
}

async fn list_identity_mappings(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<IdentityMappingListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let identity_mappings = state
        .session_store
        .list_identity_mappings_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|mapping| mapping.to_resource())
        .collect::<Vec<_>>();
    Ok(Json(IdentityMappingListResponse { identity_mappings }))
}

async fn create_identity_mapping(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertIdentityMappingRequest>,
) -> Result<(StatusCode, Json<IdentityMappingResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let mapping = state
        .session_store
        .create_identity_mapping(&principal, persist_identity_mapping_request(request))
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(mapping.to_resource())))
}

async fn get_identity_mapping(
    headers: HeaderMap,
    Path(identity_mapping_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<IdentityMappingResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let mapping = load_identity_mapping(&state, &principal, identity_mapping_id).await?;
    Ok(Json(mapping.to_resource()))
}

async fn update_identity_mapping(
    headers: HeaderMap,
    Path(identity_mapping_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertIdentityMappingRequest>,
) -> Result<Json<IdentityMappingResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let mapping = state
        .session_store
        .update_identity_mapping_for_owner(
            &principal,
            identity_mapping_id,
            persist_identity_mapping_request(request),
        )
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("identity mapping {identity_mapping_id} not found"),
                }),
            )
        })?;
    Ok(Json(mapping.to_resource()))
}

fn persist_identity_mapping_request(
    request: UpsertIdentityMappingRequest,
) -> PersistIdentityMappingRequest {
    PersistIdentityMappingRequest {
        name: request.name,
        description: request.description,
        kind: request.kind,
        issuer: request.issuer,
        external_id: request.external_id,
        claim_name: request.claim_name,
        service_principal_id: request.service_principal_id,
        project_id: request.project_id,
        labels: request.labels,
        scopes: request.scopes,
        state: request.state,
    }
}

async fn load_identity_mapping(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    identity_mapping_id: Uuid,
) -> Result<StoredIdentityMapping, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_store
        .get_identity_mapping_for_owner(principal, identity_mapping_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("identity mapping {identity_mapping_id} not found"),
                }),
            )
        })
}
