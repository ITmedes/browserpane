use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use chrono::{Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::{info, warn};
use uuid::Uuid;

use crate::auth::{AuthValidator, AuthenticatedPrincipal};
use crate::automation_task::{
    AutomationTaskEventListResponse, AutomationTaskListResponse, AutomationTaskLogListResponse,
    AutomationTaskLogStream, AutomationTaskResource, AutomationTaskSessionSource,
    AutomationTaskState, AutomationTaskTransitionRequest, PersistAutomationTaskRequest,
};
use crate::credentials::{
    CredentialBindingListResponse, CredentialBindingProvider, CredentialBindingResource,
    CredentialInjectionMode, CredentialProvider, CredentialProviderError, CredentialTotpMetadata,
    PersistCredentialBindingRequest, ResolvedWorkflowRunCredentialBindingResource,
    StoreCredentialSecretRequest, WorkflowRunCredentialBinding,
};
use crate::extension::{
    AppliedExtension, ExtensionDefinitionListResponse, ExtensionDefinitionResource,
    ExtensionVersionResource, PersistExtensionDefinitionRequest, PersistExtensionVersionRequest,
};
use crate::idle_stop::schedule_idle_session_stop;
use crate::recording::{
    prepare_session_recording_playback, FinalizeRecordingArtifactRequest,
    PreparedSessionRecordingPlayback, RecordingArtifactStore, RecordingArtifactStoreError,
    RecordingObservability, RecordingObservabilitySnapshot, RecordingPlaybackError,
    SessionRecordingPlaybackManifest, SessionRecordingPlaybackResource,
};
use crate::recording_lifecycle::{RecordingLifecycleError, RecordingLifecycleManager};
use crate::session_access::{
    SessionAutomationAccessTokenClaims, SessionAutomationAccessTokenManager,
    SessionConnectTicketManager,
};
use crate::session_control::{
    CompleteSessionRecordingRequest, CreateSessionRequest, FailSessionRecordingRequest,
    PersistCompletedSessionRecordingRequest, SessionLifecycleState, SessionListResponse,
    SessionOwnerMode, SessionRecordingFormat, SessionRecordingListResponse, SessionRecordingMode,
    SessionRecordingPolicy, SessionRecordingResource, SessionRecordingState,
    SessionRecordingTerminationReason, SessionResource, SessionStore, SessionStoreError,
    SetAutomationDelegateRequest, StoredSession, StoredSessionRecording,
};
use crate::session_hub::SessionTelemetrySnapshot;
use crate::session_manager::{SessionManager, SessionManagerError, SessionRuntime};
use crate::session_registry::SessionRegistry;
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
    validate_workflow_source_entrypoint, WorkflowObservability, WorkflowObservabilitySnapshot,
    WorkflowSource, WorkflowSourceArchive, WorkflowSourceError, WorkflowSourceResolver,
};
use crate::workflow_event_delivery::{
    group_attempts_by_delivery, PersistWorkflowEventSubscriptionRequest,
    WorkflowEventDeliveryListResponse, WorkflowEventSubscriptionListResponse,
    WorkflowEventSubscriptionResource,
};
use crate::workflow_lifecycle::WorkflowLifecycleManager;
use crate::workspaces::{
    FileWorkspaceFileListResponse, FileWorkspaceFileResource, FileWorkspaceListResponse,
    FileWorkspaceResource, PersistFileWorkspaceFileRequest, PersistFileWorkspaceRequest,
    StoreWorkspaceFileRequest, WorkspaceFileStore, WorkspaceFileStoreError,
};

mod authz;
mod automation_tasks;
mod credential_bindings;
mod extensions;
mod file_workspaces;
mod recordings;
mod resources;
mod runtime_access;
mod sessions;
mod workflow_definitions;
mod workflow_events;
mod workflow_files;
mod workflow_run_operations;
mod workflows;

use authz::*;
use resources::*;
use runtime_access::*;

/// Shared state for the HTTP API.
struct ApiState {
    registry: Arc<SessionRegistry>,
    auth_validator: Arc<AuthValidator>,
    connect_ticket_manager: Arc<SessionConnectTicketManager>,
    automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
    credential_provider: Option<Arc<CredentialProvider>>,
    recording_artifact_store: Arc<RecordingArtifactStore>,
    workspace_file_store: Arc<WorkspaceFileStore>,
    workflow_source_resolver: Arc<WorkflowSourceResolver>,
    recording_observability: Arc<RecordingObservability>,
    recording_lifecycle: Arc<RecordingLifecycleManager>,
    workflow_lifecycle: Arc<WorkflowLifecycleManager>,
    workflow_observability: Arc<WorkflowObservability>,
    workflow_log_retention: Option<ChronoDuration>,
    workflow_output_retention: Option<ChronoDuration>,
    idle_stop_timeout: std::time::Duration,
    public_gateway_url: String,
    default_owner_mode: SessionOwnerMode,
}

pub(crate) struct ApiServerConfig {
    pub bind_addr: SocketAddr,
    pub registry: Arc<SessionRegistry>,
    pub auth_validator: Arc<AuthValidator>,
    pub connect_ticket_manager: Arc<SessionConnectTicketManager>,
    pub automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    pub session_store: SessionStore,
    pub session_manager: Arc<SessionManager>,
    pub credential_provider: Option<Arc<CredentialProvider>>,
    pub recording_artifact_store: Arc<RecordingArtifactStore>,
    pub workspace_file_store: Arc<WorkspaceFileStore>,
    pub workflow_source_resolver: Arc<WorkflowSourceResolver>,
    pub recording_observability: Arc<RecordingObservability>,
    pub recording_lifecycle: Arc<RecordingLifecycleManager>,
    pub workflow_lifecycle: Arc<WorkflowLifecycleManager>,
    pub workflow_observability: Arc<WorkflowObservability>,
    pub workflow_log_retention: Option<ChronoDuration>,
    pub workflow_output_retention: Option<ChronoDuration>,
    pub idle_stop_timeout: std::time::Duration,
    pub public_gateway_url: String,
    pub default_owner_mode: SessionOwnerMode,
}

const AUTOMATION_ACCESS_TOKEN_HEADER: &str = "x-bpane-automation-access-token";
const FILE_WORKSPACE_FILE_NAME_HEADER: &str = "x-bpane-file-name";
const FILE_WORKSPACE_FILE_PROVENANCE_HEADER: &str = "x-bpane-file-provenance";
const WORKFLOW_RUN_WORKSPACE_ID_HEADER: &str = "x-bpane-workflow-workspace-id";

#[derive(Serialize)]
struct SessionStatus {
    browser_clients: u32,
    viewer_clients: u32,
    recorder_clients: u32,
    max_viewers: u32,
    viewer_slots_remaining: u32,
    exclusive_browser_owner: bool,
    mcp_owner: bool,
    resolution: (u16, u16),
    recording: SessionRecordingStatus,
    playback: SessionRecordingPlaybackResource,
    telemetry: SessionTelemetry,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
enum SessionRecordingStatusState {
    Disabled,
    Idle,
    Recording,
    Finalizing,
    Ready,
    Failed,
}

#[derive(Serialize)]
struct SessionRecordingStatus {
    configured_mode: SessionRecordingMode,
    format: SessionRecordingFormat,
    retention_sec: Option<u32>,
    state: SessionRecordingStatusState,
    active_recording_id: Option<String>,
    recorder_attached: bool,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    bytes_written: Option<u64>,
    duration_ms: Option<u64>,
}

#[derive(Serialize)]
struct SessionTelemetry {
    joins_accepted: u64,
    joins_rejected_viewer_cap: u64,
    last_join_latency_ms: u64,
    average_join_latency_ms: f64,
    max_join_latency_ms: u64,
    full_refresh_requests: u64,
    full_refresh_tiles_requested: u64,
    last_full_refresh_tiles: u64,
    max_full_refresh_tiles: u64,
    egress_send_stream_lock_acquires_total: u64,
    egress_send_stream_lock_wait_us_total: u64,
    egress_send_stream_lock_wait_us_average: f64,
    egress_send_stream_lock_wait_us_max: u64,
    egress_lagged_receives_total: u64,
    egress_lagged_frames_total: u64,
}

#[derive(Deserialize)]
struct McpOwnerRequest {
    width: u16,
    height: u16,
}

#[derive(Clone, Serialize, Deserialize)]
struct AutomationTaskSessionRequest {
    #[serde(default)]
    existing_session_id: Option<Uuid>,
    #[serde(default)]
    create_session: Option<CreateSessionRequest>,
}

#[derive(Deserialize)]
struct CreateAutomationTaskRequest {
    #[serde(default)]
    display_name: Option<String>,
    executor: String,
    session: AutomationTaskSessionRequest,
    #[serde(default)]
    input: Option<Value>,
    #[serde(default)]
    labels: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct CreateWorkflowDefinitionRequest {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    labels: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct CreateWorkflowDefinitionVersionRequest {
    version: String,
    executor: String,
    entrypoint: String,
    #[serde(default)]
    source: Option<WorkflowSource>,
    #[serde(default)]
    input_schema: Option<Value>,
    #[serde(default)]
    output_schema: Option<Value>,
    #[serde(default)]
    default_session: Option<Value>,
    #[serde(default)]
    allowed_credential_binding_ids: Vec<String>,
    #[serde(default)]
    allowed_extension_ids: Vec<String>,
    #[serde(default)]
    allowed_file_workspace_ids: Vec<String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CreateWorkflowRunRequest {
    workflow_id: Uuid,
    version: String,
    #[serde(default)]
    session: Option<AutomationTaskSessionRequest>,
    #[serde(default)]
    input: Option<Value>,
    #[serde(default)]
    source_system: Option<String>,
    #[serde(default)]
    source_reference: Option<String>,
    #[serde(default)]
    client_request_id: Option<String>,
    #[serde(default)]
    credential_binding_ids: Vec<Uuid>,
    #[serde(default)]
    workspace_inputs: Vec<CreateWorkflowRunWorkspaceInputRequest>,
    #[serde(default)]
    labels: std::collections::HashMap<String, String>,
}

#[derive(Clone, Serialize, Deserialize)]
struct CreateWorkflowRunWorkspaceInputRequest {
    workspace_id: Uuid,
    file_id: Uuid,
    #[serde(default)]
    mount_path: Option<String>,
}

#[derive(Serialize)]
struct WorkflowRunProducedFileListResponse {
    files: Vec<WorkflowRunProducedFileResource>,
}

#[derive(Deserialize)]
struct CreateFileWorkspaceRequest {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    labels: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct CreateCredentialBindingRequest {
    name: String,
    provider: CredentialBindingProvider,
    #[serde(default)]
    external_ref: Option<String>,
    #[serde(default)]
    namespace: Option<String>,
    #[serde(default)]
    allowed_origins: Vec<String>,
    injection_mode: CredentialInjectionMode,
    #[serde(default)]
    totp: Option<CredentialTotpMetadata>,
    #[serde(default)]
    secret_payload: Option<Value>,
    #[serde(default)]
    labels: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct CreateExtensionDefinitionRequest {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    labels: std::collections::HashMap<String, String>,
}

#[derive(Deserialize)]
struct CreateExtensionVersionRequest {
    version: String,
    install_path: String,
}

#[derive(Deserialize)]
struct TransitionAutomationTaskRequest {
    state: AutomationTaskState,
    #[serde(default)]
    output: Option<Value>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    artifact_refs: Vec<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Deserialize)]
struct AppendAutomationTaskLogRequest {
    stream: AutomationTaskLogStream,
    message: String,
}

#[derive(Deserialize)]
struct TransitionWorkflowRunRequest {
    state: WorkflowRunState,
    #[serde(default)]
    output: Option<Value>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    artifact_refs: Vec<String>,
    #[serde(default)]
    message: Option<String>,
    #[serde(default)]
    data: Option<Value>,
}

#[derive(Deserialize)]
struct SubmitWorkflowRunInputRequest {
    input: Value,
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    details: Option<Value>,
}

#[derive(Deserialize)]
struct ResumeWorkflowRunRequest {
    #[serde(default)]
    comment: Option<String>,
    #[serde(default)]
    details: Option<Value>,
}

#[derive(Deserialize)]
struct RejectWorkflowRunRequest {
    reason: String,
    #[serde(default)]
    details: Option<Value>,
}

#[derive(Deserialize)]
struct AppendWorkflowRunLogRequest {
    stream: AutomationTaskLogStream,
    message: String,
}

#[derive(Deserialize)]
struct CreateWorkflowEventSubscriptionRequest {
    name: String,
    target_url: String,
    event_types: Vec<String>,
    signing_secret: String,
}

#[derive(Serialize)]
struct OkResponse {
    ok: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Serialize)]
struct SessionAccessTokenResponse {
    session_id: Uuid,
    token_type: String,
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    connect: crate::session_control::SessionConnectInfo,
}

#[derive(Serialize)]
struct SessionAutomationAccessInfo {
    endpoint_url: String,
    protocol: String,
    auth_type: String,
    auth_header: String,
    status_path: String,
    mcp_owner_path: String,
    compatibility_mode: String,
}

#[derive(Serialize)]
struct SessionAutomationAccessResponse {
    session_id: Uuid,
    token_type: String,
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
    automation: SessionAutomationAccessInfo,
}

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

fn map_session_store_error(error: SessionStoreError) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        SessionStoreError::ActiveSessionConflict { .. } => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        SessionStoreError::Conflict(_) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        SessionStoreError::NotFound(_) => (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        SessionStoreError::InvalidRequest(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        SessionStoreError::Backend(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

fn map_recording_artifact_store_error(
    error: RecordingArtifactStoreError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        RecordingArtifactStoreError::InvalidSourcePath(_)
        | RecordingArtifactStoreError::InvalidReference(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        RecordingArtifactStoreError::Backend(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

fn map_workspace_file_store_error(
    error: WorkspaceFileStoreError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        WorkspaceFileStoreError::InvalidReference(_)
        | WorkspaceFileStoreError::InvalidFileName(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        WorkspaceFileStoreError::Backend(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

fn map_workspace_file_content_error(
    error: WorkspaceFileStoreError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        WorkspaceFileStoreError::Backend(inner) if inner.kind() == std::io::ErrorKind::NotFound => {
            (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: "workspace file content is no longer available".to_string(),
                }),
            )
        }
        other => map_workspace_file_store_error(other),
    }
}

fn map_credential_provider_error(
    error: CredentialProviderError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        CredentialProviderError::InvalidRequest(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        CredentialProviderError::Backend(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

fn map_workflow_source_error(error: WorkflowSourceError) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        WorkflowSourceError::Invalid(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        WorkflowSourceError::Resolve(_) | WorkflowSourceError::Materialize(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

fn map_recording_playback_error(
    error: RecordingPlaybackError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        RecordingPlaybackError::Empty => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        RecordingPlaybackError::Artifact(RecordingArtifactStoreError::Backend(inner))
            if inner.kind() == std::io::ErrorKind::NotFound =>
        {
            (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: "a playback segment artifact is no longer available".to_string(),
                }),
            )
        }
        RecordingPlaybackError::Artifact(inner) => map_recording_artifact_store_error(inner),
        RecordingPlaybackError::ManifestEncode(_)
        | RecordingPlaybackError::Io(_)
        | RecordingPlaybackError::Package(_) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

fn map_recording_lifecycle_error(
    error: RecordingLifecycleError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        RecordingLifecycleError::Disabled(_) => (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        RecordingLifecycleError::InvalidConfiguration(_)
        | RecordingLifecycleError::LaunchFailed(_)
        | RecordingLifecycleError::Store(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
    }
}

#[cfg(test)]
mod tests;
