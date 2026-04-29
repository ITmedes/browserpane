use super::*;

fn validate_session_extensions_allowed(
    workflow_version: &str,
    allowed_extension_ids: &[String],
    extensions: &[AppliedExtension],
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if extensions.is_empty() {
        return Ok(());
    }

    let allowed_ids = allowed_extension_ids
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    if allowed_ids.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "workflow definition version {workflow_version} does not allow browser extensions"
                ),
            }),
        ));
    }

    for extension in extensions {
        if !allowed_ids.contains(&extension.extension_id.to_string()) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "workflow definition version {workflow_version} does not allow extension {}",
                        extension.extension_id
                    ),
                }),
            ));
        }
    }

    Ok(())
}

async fn resolve_session_extensions(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    extension_ids: &[Uuid],
    allowed_extension_ids: Option<&[String]>,
) -> Result<Vec<AppliedExtension>, (StatusCode, Json<ErrorResponse>)> {
    if extension_ids.is_empty() {
        return Ok(Vec::new());
    }

    if !state.session_manager.profile().supports_session_extensions {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "the current runtime backend does not support session extensions"
                    .to_string(),
            }),
        ));
    }

    let allowed_set = allowed_extension_ids.map(|ids| ids.iter().cloned().collect::<HashSet<_>>());
    let mut seen_ids = HashSet::new();
    let mut extensions = Vec::with_capacity(extension_ids.len());
    for extension_id in extension_ids {
        if !seen_ids.insert(*extension_id) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("session extension {extension_id} is duplicated"),
                }),
            ));
        }

        if let Some(allowed_ids) = allowed_set.as_ref() {
            if !allowed_ids.contains(&extension_id.to_string()) {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!(
                            "workflow definition does not allow extension {extension_id}"
                        ),
                    }),
                ));
            }
        }

        let definition = state
            .session_store
            .get_extension_definition_for_owner(principal, *extension_id)
            .await
            .map_err(map_session_store_error)?
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("extension {extension_id} not found"),
                    }),
                )
            })?;
        if !definition.enabled {
            return Err((
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!("extension {extension_id} is disabled"),
                }),
            ));
        }
        let version = state
            .session_store
            .get_latest_extension_version_for_owner(principal, *extension_id)
            .await
            .map_err(map_session_store_error)?
            .ok_or_else(|| {
                (
                    StatusCode::CONFLICT,
                    Json(ErrorResponse {
                        error: format!(
                            "extension {extension_id} does not have an installed version"
                        ),
                    }),
                )
            })?;
        extensions.push(AppliedExtension {
            extension_id: definition.id,
            extension_version_id: version.id,
            name: definition.name,
            version: version.version,
            install_path: version.install_path,
        });
    }

    Ok(extensions)
}

pub(super) async fn create_owned_session(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    mut request: CreateSessionRequest,
    owner_mode: SessionOwnerMode,
    allowed_extension_ids: Option<&[String]>,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    if request.extensions.is_empty() {
        request.extensions = resolve_session_extensions(
            state,
            principal,
            &request.extension_ids,
            allowed_extension_ids,
        )
        .await?;
    }
    if !request.extensions.is_empty()
        && !state.session_manager.profile().supports_session_extensions
    {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "the current runtime backend does not support session extensions"
                    .to_string(),
            }),
        ));
    }
    if let Some(allowed_extension_ids) = allowed_extension_ids {
        validate_session_extensions_allowed(
            "session_create_payload",
            allowed_extension_ids,
            &request.extensions,
        )?;
    }
    state
        .recording_lifecycle
        .validate_mode(request.recording.mode)
        .map_err(map_recording_lifecycle_error)?;
    let stored = state
        .session_store
        .create_session(principal, request, owner_mode)
        .await
        .map_err(map_session_store_error)?;
    if let Err(error) = state
        .recording_lifecycle
        .ensure_auto_recording(&stored)
        .await
    {
        let _ = state
            .session_store
            .stop_session_for_owner(principal, stored.id)
            .await;
        state.session_manager.release(stored.id).await;
        state.registry.remove_session(stored.id).await;
        return Err(map_recording_lifecycle_error(error));
    }

    schedule_idle_session_stop(
        stored.id,
        state.idle_stop_timeout,
        state.registry.clone(),
        state.session_store.clone(),
        state.session_manager.clone(),
        state.recording_lifecycle.clone(),
    );

    Ok(stored)
}

pub(super) async fn resolve_task_session_binding(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    session: Option<AutomationTaskSessionRequest>,
    default_session: Option<&Value>,
    allowed_extension_ids: Option<&[String]>,
) -> Result<(StoredSession, AutomationTaskSessionSource), (StatusCode, Json<ErrorResponse>)> {
    match session {
        Some(AutomationTaskSessionRequest {
            existing_session_id: Some(session_id),
            create_session: None,
        }) => {
            let visible = state
                .session_store
                .get_session_for_owner(principal, session_id)
                .await
                .map_err(map_session_store_error)?
                .ok_or_else(|| {
                    (
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: format!("session {session_id} not found"),
                        }),
                    )
                })?;
            if let Some(allowed_extension_ids) = allowed_extension_ids {
                validate_session_extensions_allowed(
                    "existing_session_binding",
                    allowed_extension_ids,
                    &visible.extensions,
                )?;
            }
            Ok((visible, AutomationTaskSessionSource::ExistingSession))
        }
        Some(AutomationTaskSessionRequest {
            existing_session_id: None,
            create_session: Some(create_session_request),
        }) => {
            let owner_mode = resolve_owner_mode(state, create_session_request.owner_mode)?;
            let created = create_owned_session(
                state,
                principal,
                create_session_request,
                owner_mode,
                allowed_extension_ids,
            )
            .await?;
            Ok((created, AutomationTaskSessionSource::CreatedSession))
        }
        Some(_) => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "session must provide exactly one of existing_session_id or create_session"
                    .to_string(),
            }),
        )),
        None => {
            let Some(default_session) = default_session else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: "workflow run requires a session binding or version.default_session"
                            .to_string(),
                    }),
                ));
            };
            let create_session_request = serde_json::from_value::<CreateSessionRequest>(
                default_session.clone(),
            )
            .map_err(|error| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!(
                            "workflow version default_session is not a valid session create payload: {error}"
                        ),
                    }),
                )
            })?;
            let owner_mode = resolve_owner_mode(state, create_session_request.owner_mode)?;
            let created = create_owned_session(
                state,
                principal,
                create_session_request,
                owner_mode,
                allowed_extension_ids,
            )
            .await?;
            Ok((created, AutomationTaskSessionSource::CreatedSession))
        }
    }
}

pub(super) fn resolve_owner_mode(
    state: &ApiState,
    requested: Option<SessionOwnerMode>,
) -> Result<SessionOwnerMode, (StatusCode, Json<ErrorResponse>)> {
    let resolved = requested.unwrap_or(state.default_owner_mode);
    if resolved != state.default_owner_mode {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "owner_mode {} is not supported by the current gateway runtime",
                    resolved.as_str()
                ),
            }),
        ));
    }
    Ok(resolved)
}
