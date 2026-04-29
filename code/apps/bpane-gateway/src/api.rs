use std::collections::HashSet;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use chrono::Utc;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::{AuthValidator, AuthenticatedPrincipal};
use crate::automation_tasks::{
    AutomationTaskEventListResponse, AutomationTaskListResponse, AutomationTaskLogListResponse,
    AutomationTaskResource, AutomationTaskSessionSource, AutomationTaskState,
    AutomationTaskTransitionRequest, PersistAutomationTaskRequest,
};
use crate::credentials::{
    CredentialBindingListResponse, CredentialBindingResource, CredentialProvider,
    PersistCredentialBindingRequest, ResolvedWorkflowRunCredentialBindingResource,
    StoreCredentialSecretRequest, WorkflowRunCredentialBinding,
};
use crate::extensions::{
    AppliedExtension, ExtensionDefinitionListResponse, ExtensionDefinitionResource,
    ExtensionVersionResource, PersistExtensionDefinitionRequest, PersistExtensionVersionRequest,
};
use crate::idle_stop::schedule_idle_session_stop;
use crate::recording::{
    prepare_session_recording_playback, FinalizeRecordingArtifactRequest,
    PreparedSessionRecordingPlayback, RecordingObservabilitySnapshot,
    SessionRecordingPlaybackManifest, SessionRecordingPlaybackResource,
};
use crate::session_access::SessionAutomationAccessTokenClaims;
use crate::session_control::{
    CompleteSessionRecordingRequest, CreateSessionRequest, FailSessionRecordingRequest,
    PersistCompletedSessionRecordingRequest, SessionLifecycleState, SessionListResponse,
    SessionOwnerMode, SessionRecordingFormat, SessionRecordingListResponse, SessionRecordingMode,
    SessionRecordingPolicy, SessionRecordingResource, SessionRecordingState,
    SessionRecordingTerminationReason, SessionResource, SetAutomationDelegateRequest,
    StoredSession, StoredSessionRecording,
};
use crate::session_hub::SessionTelemetrySnapshot;
use crate::session_manager::{SessionManagerError, SessionRuntime};
use crate::workflow::{
    derive_workflow_run_admission_resource, derive_workflow_run_intervention_resource,
    derive_workflow_run_runtime_resource, PersistWorkflowDefinitionRequest,
    PersistWorkflowDefinitionVersionRequest, PersistWorkflowRunEventRequest,
    PersistWorkflowRunLogRequest, PersistWorkflowRunProducedFileRequest, PersistWorkflowRunRequest,
    StoredWorkflowDefinition, StoredWorkflowDefinitionVersion, StoredWorkflowRun,
    WorkflowDefinitionListResponse, WorkflowDefinitionResource, WorkflowDefinitionVersionResource,
    WorkflowRunEventListResponse, WorkflowRunEventResource, WorkflowRunInterventionResource,
    WorkflowRunLogListResponse, WorkflowRunLogResource, WorkflowRunProducedFileResource,
    WorkflowRunRecordingResource, WorkflowRunResource, WorkflowRunRetentionResource,
    WorkflowRunSourceSnapshot, WorkflowRunState, WorkflowRunTransitionRequest,
    WorkflowRunWorkspaceInput,
};
use crate::workflow::{
    validate_workflow_source_entrypoint, WorkflowObservabilitySnapshot, WorkflowSourceArchive,
};
use crate::workflow_event_delivery::{
    group_attempts_by_delivery, PersistWorkflowEventSubscriptionRequest,
    WorkflowEventDeliveryListResponse, WorkflowEventSubscriptionListResponse,
    WorkflowEventSubscriptionResource,
};
use crate::workspaces::{
    FileWorkspaceFileListResponse, FileWorkspaceFileResource, FileWorkspaceListResponse,
    FileWorkspaceResource, PersistFileWorkspaceFileRequest, PersistFileWorkspaceRequest,
    StoreWorkspaceFileRequest, WorkspaceFileStoreError,
};

mod authz;
mod automation_tasks;
mod credential_bindings;
mod errors;
mod extensions;
mod file_workspaces;
mod recordings;
mod resources;
mod runtime_access;
mod sessions;
mod types;
mod workflow_definitions;
mod workflow_events;
mod workflow_files;
mod workflow_run_operations;
mod workflows;

use authz::*;
use errors::*;
use resources::*;
use runtime_access::*;
pub(crate) use types::ApiServerConfig;
use types::*;

fn require_credential_provider(
    state: &ApiState,
) -> Result<&CredentialProvider, (StatusCode, Json<ErrorResponse>)> {
    state.credential_provider.as_deref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: "credential bindings are not configured on this gateway".to_string(),
            }),
        )
    })
}

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

async fn get_workflow_operations(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowObservabilitySnapshot>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    Ok(Json(state.workflow_observability.snapshot().await))
}

/// Runs the HTTP API server for MCP bridge communication.
pub async fn run_api_server(config: ApiServerConfig) -> anyhow::Result<()> {
    let bind_addr = config.bind_addr;
    let state = Arc::new(ApiState {
        registry: config.registry,
        auth_validator: config.auth_validator,
        connect_ticket_manager: config.connect_ticket_manager,
        automation_access_token_manager: config.automation_access_token_manager,
        session_store: config.session_store,
        session_manager: config.session_manager,
        credential_provider: config.credential_provider,
        recording_artifact_store: config.recording_artifact_store,
        workspace_file_store: config.workspace_file_store,
        workflow_source_resolver: config.workflow_source_resolver,
        recording_observability: config.recording_observability,
        recording_lifecycle: config.recording_lifecycle,
        workflow_lifecycle: config.workflow_lifecycle,
        workflow_observability: config.workflow_observability,
        workflow_log_retention: config.workflow_log_retention,
        workflow_output_retention: config.workflow_output_retention,
        idle_stop_timeout: config.idle_stop_timeout,
        public_gateway_url: config.public_gateway_url,
        default_owner_mode: config.default_owner_mode,
    });

    let app = build_api_router(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    info!("HTTP API listening on {bind_addr}");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn create_owned_session(
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

async fn resolve_task_session_binding(
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

fn required_header_string(
    headers: &HeaderMap,
    name: &str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let value = headers
        .get(name)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("missing required header {name}"),
                }),
            )
        })?
        .to_str()
        .map_err(|_| {
            (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("header {name} must be valid UTF-8"),
                }),
            )
        })?
        .trim()
        .to_string();
    if value.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("header {name} must not be empty"),
            }),
        ));
    }
    Ok(value)
}

fn parse_optional_json_object_header(
    headers: &HeaderMap,
    name: &str,
) -> Result<Option<Value>, (StatusCode, Json<ErrorResponse>)> {
    let Some(raw) = headers.get(name) else {
        return Ok(None);
    };
    let raw = raw.to_str().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("header {name} must be valid UTF-8"),
            }),
        )
    })?;
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    let value = serde_json::from_str::<Value>(trimmed).map_err(|error| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("header {name} must contain valid JSON: {error}"),
            }),
        )
    })?;
    if !value.is_object() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: format!("header {name} must contain a JSON object"),
            }),
        ));
    }
    Ok(Some(value))
}

fn header_value_or_default(value: &str, fallback: &'static str) -> HeaderValue {
    HeaderValue::from_str(value).unwrap_or_else(|_| HeaderValue::from_static(fallback))
}

fn sanitize_content_disposition_filename(file_name: &str) -> String {
    file_name.replace(['"', '\\'], "_")
}

fn build_api_router(state: Arc<ApiState>) -> Router {
    Router::new()
        .merge(sessions::session_routes())
        .merge(extensions::extension_routes())
        .merge(credential_bindings::credential_binding_routes())
        .merge(file_workspaces::file_workspace_routes())
        .merge(workflow_events::workflow_event_subscription_routes())
        .merge(workflow_definitions::workflow_definition_routes())
        .merge(workflows::workflow_routes())
        .merge(workflow_files::workflow_file_routes())
        .merge(credential_bindings::workflow_run_credential_binding_routes())
        .merge(workflow_run_operations::workflow_run_operation_routes())
        .merge(workflow_events::workflow_run_event_routes())
        .merge(automation_tasks::automation_task_routes())
        .merge(recordings::recording_routes())
        .merge(sessions::session_operation_routes())
        .merge(recordings::recording_operation_routes())
        .route("/api/v1/workflow/operations", get(get_workflow_operations))
        .merge(sessions::legacy_session_routes())
        .with_state(state)
}

fn resolve_owner_mode(
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

#[cfg(test)]
mod tests;
