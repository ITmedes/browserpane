use axum::routing::{get, post};

use super::*;

pub(super) fn service_principal_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/service-principals",
            post(create_service_principal).get(list_service_principals),
        )
        .route(
            "/api/v1/service-principals/{service_principal_id}",
            get(get_service_principal).put(update_service_principal),
        )
}

async fn list_service_principals(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ServicePrincipalListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let service_principals = state
        .session_store
        .list_service_principals_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|service_principal| service_principal.to_resource())
        .collect::<Vec<_>>();
    Ok(Json(ServicePrincipalListResponse { service_principals }))
}

async fn create_service_principal(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertServicePrincipalRequest>,
) -> Result<(StatusCode, Json<ServicePrincipalResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let service_principal = state
        .session_store
        .create_service_principal(&principal, persist_service_principal_request(request))
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(service_principal.to_resource())))
}

async fn get_service_principal(
    headers: HeaderMap,
    Path(service_principal_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<ServicePrincipalResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let service_principal =
        load_service_principal(&state, &principal, service_principal_id).await?;
    Ok(Json(service_principal.to_resource()))
}

async fn update_service_principal(
    headers: HeaderMap,
    Path(service_principal_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<UpsertServicePrincipalRequest>,
) -> Result<Json<ServicePrincipalResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let service_principal = state
        .session_store
        .update_service_principal_for_owner(
            &principal,
            service_principal_id,
            persist_service_principal_request(request),
        )
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("service principal {service_principal_id} not found"),
                }),
            )
        })?;
    Ok(Json(service_principal.to_resource()))
}

fn persist_service_principal_request(
    request: UpsertServicePrincipalRequest,
) -> PersistServicePrincipalRequest {
    PersistServicePrincipalRequest {
        name: request.name,
        description: request.description,
        client_id: request.client_id,
        issuer: request.issuer,
        labels: request.labels,
        scopes: request.scopes,
        allowed_project_ids: request.allowed_project_ids,
        state: request.state,
    }
}

async fn load_service_principal(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    service_principal_id: Uuid,
) -> Result<StoredServicePrincipal, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_store
        .get_service_principal_for_owner(principal, service_principal_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("service principal {service_principal_id} not found"),
                }),
            )
        })
}
