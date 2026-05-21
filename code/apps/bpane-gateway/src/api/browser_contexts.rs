use std::collections::{HashMap, HashSet};

use axum::routing::{get, post};

use super::*;
use crate::session_control::{BrowserContextUsageResource, StoredBrowserContext};

pub(super) fn browser_context_routes() -> Router<Arc<ApiState>> {
    Router::new()
        .route(
            "/api/v1/browser-contexts",
            post(create_browser_context).get(list_browser_contexts),
        )
        .route(
            "/api/v1/browser-contexts/{context_id}/clone",
            post(clone_browser_context),
        )
        .route(
            "/api/v1/browser-contexts/{context_id}",
            get(get_browser_context).delete(delete_browser_context),
        )
}

async fn list_browser_contexts(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<BrowserContextListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let contexts = state
        .session_store
        .list_browser_contexts_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?;
    let contexts = browser_context_resources_with_usage(&state, &principal, contexts).await?;
    Ok(Json(BrowserContextListResponse { contexts }))
}

async fn create_browser_context(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateBrowserContextRequest>,
) -> Result<(StatusCode, Json<BrowserContextResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let context = state
        .session_store
        .create_browser_context(
            &principal,
            PersistBrowserContextRequest {
                id: None,
                name: request.name,
                description: request.description,
                labels: request.labels,
                persistence_mode: request.persistence_mode,
                retention_sec: request.retention_sec,
                max_profile_storage_bytes: request.max_profile_storage_bytes,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(context.to_resource())))
}

async fn clone_browser_context(
    headers: HeaderMap,
    Path(context_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CloneBrowserContextRequest>,
) -> Result<(StatusCode, Json<BrowserContextResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let source = state
        .session_store
        .get_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    if source.state != BrowserContextState::Ready {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("browser context {context_id} is deleted and cannot be cloned"),
            }),
        ));
    }
    if source.persistence_mode != BrowserContextPersistenceMode::Reusable {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("browser context {context_id} is not reusable and cannot be cloned"),
            }),
        ));
    }
    if let Some(active_session_id) = state
        .session_manager
        .active_browser_context_session_id(context_id)
        .await
    {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "browser context {context_id} is already used by active session {active_session_id}"
                ),
            }),
        ));
    }

    let target_context_id = Uuid::now_v7();
    let target_request = PersistBrowserContextRequest {
        id: Some(target_context_id),
        name: request.name,
        description: request.description.or_else(|| source.description.clone()),
        labels: request.labels.unwrap_or_else(|| source.labels.clone()),
        persistence_mode: BrowserContextPersistenceMode::Reusable,
        retention_sec: request.retention_sec.or(source.retention_sec),
        max_profile_storage_bytes: request
            .max_profile_storage_bytes
            .or(source.max_profile_storage_bytes),
    };
    SessionStore::validate_browser_context_request(&target_request)
        .map_err(map_session_store_error)?;

    state
        .session_manager
        .clone_browser_context_data(source.id, target_context_id)
        .await
        .map_err(|error| {
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: error.to_string(),
                }),
            )
        })?;

    let context = match state
        .session_store
        .create_browser_context(&principal, target_request)
        .await
    {
        Ok(context) => context,
        Err(error) => {
            if let Err(cleanup_error) = state
                .session_manager
                .delete_browser_context_data(target_context_id)
                .await
            {
                warn!(
                    source_browser_context_id = %source.id,
                    target_browser_context_id = %target_context_id,
                    error = %cleanup_error,
                    "failed to clean up cloned browser context data after metadata persistence failure",
                );
            }
            return Err(map_session_store_error(error));
        }
    };

    Ok((
        StatusCode::CREATED,
        Json(browser_context_resource_with_usage(&state, &principal, context).await?),
    ))
}

async fn get_browser_context(
    headers: HeaderMap,
    Path(context_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<BrowserContextResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let context = state
        .session_store
        .get_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    Ok(Json(
        browser_context_resource_with_usage(&state, &principal, context).await?,
    ))
}

async fn delete_browser_context(
    headers: HeaderMap,
    Path(context_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<BrowserContextResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let existing = state
        .session_store
        .get_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    if existing.state != BrowserContextState::Deleted {
        state
            .session_manager
            .delete_browser_context_data(context_id)
            .await
            .map_err(|error| {
                (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: error.to_string(),
                    }),
                )
            })?;
    }
    let context = state
        .session_store
        .delete_browser_context_for_owner(&principal, context_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("browser context {context_id} not found"),
                }),
            )
        })?;
    Ok(Json(
        browser_context_resource_with_usage(&state, &principal, context).await?,
    ))
}

async fn browser_context_resources_with_usage(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    contexts: Vec<StoredBrowserContext>,
) -> Result<Vec<BrowserContextResource>, (StatusCode, Json<ErrorResponse>)> {
    let context_ids = contexts
        .iter()
        .map(|context| context.id)
        .collect::<Vec<_>>();
    let usage_by_context = browser_context_usage_by_id(state, principal, &context_ids).await?;
    Ok(contexts
        .into_iter()
        .map(|context| {
            let usage = usage_by_context
                .get(&context.id)
                .cloned()
                .unwrap_or_default();
            browser_context_resource_with_usage_value(context, usage)
        })
        .collect())
}

async fn browser_context_resource_with_usage(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    context: StoredBrowserContext,
) -> Result<BrowserContextResource, (StatusCode, Json<ErrorResponse>)> {
    let mut usage_by_context = browser_context_usage_by_id(state, principal, &[context.id]).await?;
    let usage = usage_by_context.remove(&context.id).unwrap_or_default();
    Ok(browser_context_resource_with_usage_value(context, usage))
}

fn browser_context_resource_with_usage_value(
    context: StoredBrowserContext,
    mut usage: BrowserContextUsageResource,
) -> BrowserContextResource {
    let mut resource = context.to_resource();
    usage.profile_storage_limit_exceeded = match (
        resource.max_profile_storage_bytes,
        usage.profile_storage_bytes,
    ) {
        (Some(limit), Some(bytes)) => bytes > limit,
        _ => false,
    };
    resource.usage = usage;
    resource
}

async fn browser_context_usage_by_id(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    context_ids: &[Uuid],
) -> Result<HashMap<Uuid, BrowserContextUsageResource>, (StatusCode, Json<ErrorResponse>)> {
    if context_ids.is_empty() {
        return Ok(HashMap::new());
    }
    let requested_context_ids = context_ids.iter().copied().collect::<HashSet<_>>();
    let mut usage_by_context = HashMap::new();
    let storage_by_context = state
        .session_manager
        .browser_context_profile_storage_bytes(context_ids)
        .await
        .unwrap_or_else(|error| {
            warn!(
                error = %error,
                "could not inspect browser context profile storage usage",
            );
            HashMap::new()
        });
    let sessions = state
        .session_store
        .list_sessions_for_owner(principal)
        .await
        .map_err(map_session_store_error)?;

    for session in sessions {
        let Some(context_id) = reusable_context_id(&session) else {
            continue;
        };
        if !requested_context_ids.contains(&context_id) {
            continue;
        }
        usage_by_context
            .entry(context_id)
            .or_insert_with(BrowserContextUsageResource::default)
            .visible_session_count += 1;
    }

    for context_id in requested_context_ids {
        let Some(active_session_id) = state
            .session_manager
            .active_browser_context_session_id(context_id)
            .await
        else {
            continue;
        };
        let usage = usage_by_context
            .entry(context_id)
            .or_insert_with(BrowserContextUsageResource::default);
        usage.active_runtime_session_count = 1;
        usage.active_runtime_session_id = Some(active_session_id);
    }

    for (context_id, storage_bytes) in storage_by_context {
        let usage = usage_by_context
            .entry(context_id)
            .or_insert_with(BrowserContextUsageResource::default);
        usage.profile_storage_bytes = Some(storage_bytes);
    }

    Ok(usage_by_context)
}

fn reusable_context_id(session: &StoredSession) -> Option<Uuid> {
    (session.browser_context.mode == SessionBrowserContextMode::Reusable)
        .then_some(session.browser_context.context_id)
        .flatten()
}
