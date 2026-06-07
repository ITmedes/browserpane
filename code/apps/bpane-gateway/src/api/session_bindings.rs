use crate::session_control::validate_egress_profile_project_scope;

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

pub(super) async fn resolve_session_template_defaults(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    request: CreateSessionRequest,
) -> Result<CreateSessionRequest, (StatusCode, Json<ErrorResponse>)> {
    let Some(template_id) = request.template_id.clone() else {
        return Ok(request);
    };
    let Ok(template_uuid) = Uuid::parse_str(&template_id) else {
        return Ok(request);
    };
    let Some(template) = state
        .session_store
        .get_session_template_for_owner(principal, template_uuid)
        .await
        .map_err(map_session_store_error)?
    else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("session template {template_id} not found"),
            }),
        ));
    };

    Ok(merge_template_defaults(request, template.defaults))
}

fn merge_template_defaults(
    mut request: CreateSessionRequest,
    defaults: SessionTemplateDefaults,
) -> CreateSessionRequest {
    request.project_id = request.project_id.or(defaults.project_id);
    request.owner_mode = request.owner_mode.or(defaults.owner_mode);
    if request.viewport.is_none() {
        request.viewport = defaults.viewport;
    }
    request.idle_timeout_sec = request.idle_timeout_sec.or(defaults.idle_timeout_sec);

    let mut labels = defaults.labels;
    labels.extend(request.labels);
    request.labels = labels;

    request.integration_context =
        merge_integration_context(defaults.integration_context, request.integration_context);
    request.network_identity =
        merge_network_identity(defaults.network_identity, request.network_identity);

    if request.recording == SessionRecordingPolicy::default() {
        if let Some(recording) = defaults.recording {
            request.recording = recording;
        }
    }

    request
}

fn merge_network_identity(
    defaults: Option<SessionNetworkIdentity>,
    override_value: Option<SessionNetworkIdentity>,
) -> Option<SessionNetworkIdentity> {
    match (defaults, override_value) {
        (Some(mut defaults), Some(override_value)) => {
            defaults.locale = override_value.locale.or(defaults.locale);
            if !override_value.languages.is_empty() {
                defaults.languages = override_value.languages;
            }
            defaults.timezone = override_value.timezone.or(defaults.timezone);
            defaults.geolocation = override_value.geolocation.or(defaults.geolocation);
            defaults.user_agent = override_value.user_agent.or(defaults.user_agent);
            defaults.browser_identity = override_value
                .browser_identity
                .or(defaults.browser_identity);
            defaults.egress_profile_id = override_value
                .egress_profile_id
                .or(defaults.egress_profile_id);
            Some(defaults)
        }
        (_, Some(value)) => Some(value),
        (Some(value), None) => Some(value),
        (None, None) => None,
    }
}

fn merge_integration_context(
    defaults: Option<Value>,
    override_value: Option<Value>,
) -> Option<Value> {
    match (defaults, override_value) {
        (Some(Value::Object(mut default_object)), Some(Value::Object(override_object))) => {
            default_object.extend(override_object);
            Some(Value::Object(default_object))
        }
        (_, Some(value)) => Some(value),
        (Some(value), None) => Some(value),
        (None, None) => None,
    }
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
    let reusable_context = validate_session_browser_context(state, principal, &request).await?;
    validate_session_egress_profile(state, principal, &request).await?;
    if let Some(context) = reusable_context {
        enforce_browser_context_storage_limit(state, &context).await?;
        state
            .session_store
            .mark_browser_context_used_for_owner(principal, context.id)
            .await
            .map_err(map_session_store_error)?
            .ok_or_else(|| {
                (
                    StatusCode::NOT_FOUND,
                    Json(ErrorResponse {
                        error: format!("browser context {} not found", context.id),
                    }),
                )
            })?;
    }
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

async fn validate_session_egress_profile(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    request: &CreateSessionRequest,
) -> Result<Option<StoredEgressProfile>, (StatusCode, Json<ErrorResponse>)> {
    let Some(profile_id) = request
        .network_identity
        .as_ref()
        .and_then(|identity| identity.egress_profile_id)
    else {
        return Ok(None);
    };
    let profile = state
        .session_store
        .get_egress_profile_for_owner(principal, profile_id)
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
    if profile.state == EgressProfileState::Disabled {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("egress profile {profile_id} is disabled"),
            }),
        ));
    }
    validate_egress_profile_project_scope(request.project_id, profile.id, profile.project_id)
        .map_err(map_session_store_error)?;
    Ok(Some(profile))
}

async fn validate_session_browser_context(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    request: &CreateSessionRequest,
) -> Result<Option<StoredBrowserContext>, (StatusCode, Json<ErrorResponse>)> {
    let Some(browser_context) = &request.browser_context else {
        return Ok(None);
    };
    if browser_context.mode != SessionBrowserContextMode::Reusable {
        return Ok(None);
    }
    let Some(context_id) = browser_context.context_id else {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "browser_context.context_id is required for reusable mode".to_string(),
            }),
        ));
    };
    let context = state
        .session_store
        .get_browser_context_for_owner(principal, context_id)
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
    if context.state != BrowserContextState::Ready {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("browser context {context_id} is not ready"),
            }),
        ));
    }
    if context.persistence_mode != BrowserContextPersistenceMode::Reusable {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("browser context {context_id} is not reusable"),
            }),
        ));
    }
    if let Some(context_project_id) = context.project_id {
        if request.project_id != Some(context_project_id) {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "browser context {context_id} belongs to project {context_project_id} and requires a matching session project_id"
                    ),
                }),
            ));
        }
    }
    Ok(Some(context))
}

async fn enforce_browser_context_storage_limit(
    state: &ApiState,
    context: &StoredBrowserContext,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let Some(limit) = context.max_profile_storage_bytes else {
        return Ok(());
    };
    let storage_by_context = state
        .session_manager
        .browser_context_profile_storage_bytes(&[context.id])
        .await
        .map_err(|error| {
            (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!(
                        "could not inspect browser context {} storage for quota enforcement: {error}",
                        context.id
                    ),
                }),
            )
        })?;
    let storage_bytes = storage_by_context.get(&context.id).copied().unwrap_or(0);
    if storage_bytes > limit {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "browser context {} profile storage {} bytes exceeds configured limit {} bytes",
                    context.id, storage_bytes, limit
                ),
            }),
        ));
    }
    Ok(())
}

pub(super) async fn resolve_task_session_binding(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    session: Option<AutomationTaskSessionRequest>,
    default_session: Option<&Value>,
    allowed_extension_ids: Option<&[String]>,
    required_project_id: Option<Uuid>,
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
            validate_task_session_project(&visible, required_project_id)?;
            Ok((visible, AutomationTaskSessionSource::ExistingSession))
        }
        Some(AutomationTaskSessionRequest {
            existing_session_id: None,
            create_session: Some(create_session_request),
        }) => {
            let create_session_request =
                apply_task_session_project(create_session_request, required_project_id)?;
            let create_session_request =
                resolve_session_template_defaults(state, principal, create_session_request).await?;
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
            let create_session_request =
                apply_task_session_project(create_session_request, required_project_id)?;
            let create_session_request =
                resolve_session_template_defaults(state, principal, create_session_request).await?;
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

fn validate_task_session_project(
    session: &StoredSession,
    required_project_id: Option<Uuid>,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let Some(project_id) = required_project_id else {
        return Ok(());
    };
    if session.project_id != Some(project_id) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!(
                    "workflow run project_id {project_id} must match the bound session project_id"
                ),
            }),
        ));
    }
    Ok(())
}

fn apply_task_session_project(
    mut request: CreateSessionRequest,
    required_project_id: Option<Uuid>,
) -> Result<CreateSessionRequest, (StatusCode, Json<ErrorResponse>)> {
    let Some(project_id) = required_project_id else {
        return Ok(request);
    };
    if let Some(existing_project_id) = request.project_id {
        if existing_project_id != project_id {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!(
                        "workflow run project_id {project_id} must match create_session.project_id {existing_project_id}"
                    ),
                }),
            ));
        }
    } else {
        request.project_id = Some(project_id);
    }
    Ok(request)
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
