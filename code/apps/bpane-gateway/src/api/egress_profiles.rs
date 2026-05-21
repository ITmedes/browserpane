use axum::routing::{get, post};

use super::*;

pub(super) fn egress_profile_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/egress-profiles",
            post(create_egress_profile).get(list_egress_profiles),
        )
        .route(
            "/api/v1/egress-profiles/{profile_id}",
            get(get_egress_profile),
        )
}

async fn list_egress_profiles(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<EgressProfileListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let profiles = state
        .session_store
        .list_egress_profiles_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|profile| profile.to_resource())
        .collect();
    Ok(Json(EgressProfileListResponse { profiles }))
}

async fn create_egress_profile(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateEgressProfileRequest>,
) -> Result<(StatusCode, Json<EgressProfileResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let profile = state
        .session_store
        .create_egress_profile(&principal, persist_egress_profile_request(request))
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(profile.to_resource())))
}

async fn get_egress_profile(
    headers: HeaderMap,
    Path(profile_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<EgressProfileResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let profile = state
        .session_store
        .get_egress_profile_for_owner(&principal, profile_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("egress profile {profile_id} not found"),
                }),
            )
        })?;
    Ok(Json(profile.to_resource()))
}

fn persist_egress_profile_request(
    request: CreateEgressProfileRequest,
) -> PersistEgressProfileRequest {
    PersistEgressProfileRequest {
        name: request.name,
        description: request.description,
        labels: request.labels,
        proxy: request.proxy,
        bypass_rules: request.bypass_rules,
        custom_ca: request.custom_ca,
        traffic_observation: request.traffic_observation,
        state: request.state,
    }
}
