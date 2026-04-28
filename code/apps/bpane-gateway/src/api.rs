use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::{Component, Path as FsPath};
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
use crate::automation_access_token::{
    SessionAutomationAccessTokenClaims, SessionAutomationAccessTokenManager,
};
use crate::automation_task::{
    AutomationTaskEventListResponse, AutomationTaskListResponse, AutomationTaskLogListResponse,
    AutomationTaskLogStream, AutomationTaskResource, AutomationTaskSessionSource,
    AutomationTaskState, AutomationTaskTransitionRequest, PersistAutomationTaskRequest,
};
use crate::connect_ticket::SessionConnectTicketManager;
use crate::credential_binding::{
    CredentialBindingListResponse, CredentialBindingProvider, CredentialBindingResource,
    CredentialInjectionMode, CredentialTotpMetadata, PersistCredentialBindingRequest,
    ResolvedWorkflowRunCredentialBindingResource, WorkflowRunCredentialBinding,
};
use crate::credential_provider::{
    CredentialProvider, CredentialProviderError, StoreCredentialSecretRequest,
};
use crate::extension::{
    AppliedExtension, ExtensionDefinitionListResponse, ExtensionDefinitionResource,
    ExtensionVersionResource, PersistExtensionDefinitionRequest, PersistExtensionVersionRequest,
};
use crate::file_workspace::{
    FileWorkspaceFileListResponse, FileWorkspaceFileResource, FileWorkspaceListResponse,
    FileWorkspaceResource, PersistFileWorkspaceFileRequest, PersistFileWorkspaceRequest,
};
use crate::idle_stop::schedule_idle_session_stop;
use crate::recording_artifact_store::{
    FinalizeRecordingArtifactRequest, RecordingArtifactStore, RecordingArtifactStoreError,
};
use crate::recording_lifecycle::{RecordingLifecycleError, RecordingLifecycleManager};
use crate::recording_observability::{RecordingObservability, RecordingObservabilitySnapshot};
use crate::recording_playback::{
    prepare_session_recording_playback, PreparedSessionRecordingPlayback, RecordingPlaybackError,
    SessionRecordingPlaybackManifest, SessionRecordingPlaybackResource,
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
use crate::workflow_event_delivery::{
    group_attempts_by_delivery, PersistWorkflowEventSubscriptionRequest,
    WorkflowEventDeliveryListResponse, WorkflowEventSubscriptionListResponse,
    WorkflowEventSubscriptionResource,
};
use crate::workflow_lifecycle::WorkflowLifecycleManager;
use crate::workflow_observability::{WorkflowObservability, WorkflowObservabilitySnapshot};
use crate::workflow_source::{
    validate_workflow_source_entrypoint, WorkflowSource, WorkflowSourceArchive,
    WorkflowSourceError, WorkflowSourceResolver,
};
use crate::workspace_file_store::{
    StoreWorkspaceFileRequest, WorkspaceFileStore, WorkspaceFileStoreError,
};

mod authz;
mod automation_tasks;
mod credential_bindings;
mod extensions;
mod file_workspaces;
mod recordings;
mod runtime_access;
mod sessions;
mod workflow_events;
mod workflow_run_operations;
mod workflows;

use authz::*;
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

fn session_status_from_snapshot(
    snapshot: SessionTelemetrySnapshot,
    recording_policy: &SessionRecordingPolicy,
    latest_recording: Option<&StoredSessionRecording>,
    playback: SessionRecordingPlaybackResource,
) -> SessionStatus {
    SessionStatus {
        browser_clients: snapshot.browser_clients,
        viewer_clients: snapshot.viewer_clients,
        recorder_clients: snapshot.recorder_clients,
        max_viewers: snapshot.max_viewers,
        viewer_slots_remaining: snapshot.viewer_slots_remaining,
        exclusive_browser_owner: snapshot.exclusive_browser_owner,
        mcp_owner: snapshot.mcp_owner,
        resolution: snapshot.resolution,
        recording: recording_status_from_snapshot(snapshot, recording_policy, latest_recording),
        playback,
        telemetry: SessionTelemetry {
            joins_accepted: snapshot.joins_accepted,
            joins_rejected_viewer_cap: snapshot.joins_rejected_viewer_cap,
            last_join_latency_ms: snapshot.last_join_latency_ms,
            average_join_latency_ms: snapshot.average_join_latency_ms,
            max_join_latency_ms: snapshot.max_join_latency_ms,
            full_refresh_requests: snapshot.full_refresh_requests,
            full_refresh_tiles_requested: snapshot.full_refresh_tiles_requested,
            last_full_refresh_tiles: snapshot.last_full_refresh_tiles,
            max_full_refresh_tiles: snapshot.max_full_refresh_tiles,
            egress_send_stream_lock_acquires_total: snapshot.egress_send_stream_lock_acquires_total,
            egress_send_stream_lock_wait_us_total: snapshot.egress_send_stream_lock_wait_us_total,
            egress_send_stream_lock_wait_us_average: snapshot
                .egress_send_stream_lock_wait_us_average,
            egress_send_stream_lock_wait_us_max: snapshot.egress_send_stream_lock_wait_us_max,
            egress_lagged_receives_total: snapshot.egress_lagged_receives_total,
            egress_lagged_frames_total: snapshot.egress_lagged_frames_total,
        },
    }
}

fn recording_status_from_snapshot(
    snapshot: SessionTelemetrySnapshot,
    recording_policy: &SessionRecordingPolicy,
    latest_recording: Option<&StoredSessionRecording>,
) -> SessionRecordingStatus {
    let active_recording_id = latest_recording
        .filter(|recording| recording.state.is_active())
        .map(|recording| recording.id.to_string());
    let state = if let Some(recording) = latest_recording {
        match recording.state {
            SessionRecordingState::Starting | SessionRecordingState::Recording => {
                SessionRecordingStatusState::Recording
            }
            SessionRecordingState::Finalizing => SessionRecordingStatusState::Finalizing,
            SessionRecordingState::Ready => SessionRecordingStatusState::Ready,
            SessionRecordingState::Failed => SessionRecordingStatusState::Failed,
        }
    } else if recording_policy.mode == SessionRecordingMode::Disabled {
        SessionRecordingStatusState::Disabled
    } else if snapshot.recorder_clients > 0 {
        SessionRecordingStatusState::Recording
    } else {
        SessionRecordingStatusState::Idle
    };

    SessionRecordingStatus {
        configured_mode: recording_policy.mode,
        format: recording_policy.format,
        retention_sec: recording_policy.retention_sec,
        state,
        active_recording_id,
        recorder_attached: snapshot.recorder_clients > 0,
        started_at: latest_recording.map(|recording| recording.started_at),
        bytes_written: latest_recording.and_then(|recording| recording.bytes),
        duration_ms: latest_recording.and_then(|recording| recording.duration_ms),
    }
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

fn session_resource(
    state: &ApiState,
    stored: &StoredSession,
    state_override: Option<SessionLifecycleState>,
) -> SessionResource {
    stored.to_resource(
        &state.public_gateway_url,
        state
            .session_manager
            .describe_session_runtime(stored.id)
            .into(),
        state_override,
    )
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
        .merge(workflows::workflow_routes())
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

async fn load_session_recording(
    state: &ApiState,
    session_id: Uuid,
    recording_id: Uuid,
) -> Result<StoredSessionRecording, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_store
        .get_recording_for_session(session_id, recording_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "recording {recording_id} was not found for session {session_id}"
                    ),
                }),
            )
        })
}

async fn load_session_recording_playback(
    state: &ApiState,
    session_id: Uuid,
) -> Result<PreparedSessionRecordingPlayback, (StatusCode, Json<ErrorResponse>)> {
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    Ok(prepare_session_recording_playback(
        session_id,
        &recordings,
        Utc::now(),
    ))
}

fn latest_recording(recordings: &[StoredSessionRecording]) -> Option<&StoredSessionRecording> {
    recordings.iter().max_by(|left, right| {
        left.updated_at
            .cmp(&right.updated_at)
            .then_with(|| left.created_at.cmp(&right.created_at))
    })
}

async fn build_workflow_run_resource(
    state: &ApiState,
    run: &StoredWorkflowRun,
) -> Result<WorkflowRunResource, (StatusCode, Json<ErrorResponse>)> {
    let recordings = state
        .session_store
        .list_recordings_for_session(run.session_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .filter(|recording| workflow_run_recording_matches(run, recording, Utc::now()))
        .map(workflow_run_recording_resource)
        .collect::<Vec<_>>();
    let events = workflow_run_event_resources(state, run).await?;
    let admission = derive_workflow_run_admission_resource(run.state, &events);
    let intervention = derive_workflow_run_intervention_resource(run.state, &events);
    let session_state = state
        .session_store
        .get_session_by_id(run.session_id)
        .await
        .map_err(map_session_store_error)?
        .map(|session| session.state);
    let runtime = derive_workflow_run_runtime_resource(run.state, session_state, &events);
    Ok(run.to_resource(
        recordings,
        workflow_run_retention_resource(state, run),
        admission,
        intervention,
        runtime,
    ))
}

async fn workflow_run_event_resources(
    state: &ApiState,
    run: &StoredWorkflowRun,
) -> Result<Vec<WorkflowRunEventResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = load_session_owner_principal(state, run.session_id).await?;
    let mut events = state
        .session_store
        .list_workflow_run_events_for_owner(&principal, run.id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|event| event.to_resource())
        .collect::<Vec<WorkflowRunEventResource>>();
    let task_events = state
        .session_store
        .list_automation_task_events_for_owner(&principal, run.automation_task_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|event| {
            WorkflowRunEventResource::from_automation_task(run.id, run.automation_task_id, &event)
        });
    events.extend(task_events);
    events.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(events)
}

async fn workflow_run_intervention_resource(
    state: &ApiState,
    run: &StoredWorkflowRun,
) -> Result<WorkflowRunInterventionResource, (StatusCode, Json<ErrorResponse>)> {
    let events = workflow_run_event_resources(state, run).await?;
    Ok(derive_workflow_run_intervention_resource(
        run.state, &events,
    ))
}

async fn load_owner_workflow_run(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    run_id: Uuid,
) -> Result<StoredWorkflowRun, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_store
        .get_workflow_run_for_owner(principal, run_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow run {run_id} not found"),
                }),
            )
        })
}

fn ensure_run_awaiting_input(
    run: &StoredWorkflowRun,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if run.state != WorkflowRunState::AwaitingInput {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("workflow run {} is not awaiting input", run.id),
            }),
        ));
    }
    Ok(())
}

fn workflow_run_intervention_resolution_data(
    request_id: Option<Uuid>,
    action: &str,
    input: Option<Value>,
    reason: Option<String>,
    principal: &AuthenticatedPrincipal,
    details: Option<Value>,
) -> Value {
    serde_json::json!({
        "intervention_resolution": {
            "request_id": request_id.map(|value| value.to_string()),
            "action": action,
            "input": input,
            "reason": reason,
            "actor_subject": principal.subject,
            "actor_issuer": principal.issuer,
            "actor_display_name": principal.display_name,
            "details": details
        }
    })
}

fn trim_optional_comment(
    comment: Option<String>,
) -> Result<Option<String>, (StatusCode, Json<ErrorResponse>)> {
    match comment {
        Some(comment) if comment.trim().is_empty() => Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "comment must not be empty when provided".to_string(),
            }),
        )),
        Some(comment) => Ok(Some(comment.trim().to_string())),
        None => Ok(None),
    }
}

fn workflow_run_recording_matches(
    run: &StoredWorkflowRun,
    recording: &StoredSessionRecording,
    now: chrono::DateTime<chrono::Utc>,
) -> bool {
    let run_started_at = run.started_at.unwrap_or(run.created_at);
    let run_ended_at = run.completed_at.unwrap_or(now);
    let recording_ended_at = recording.completed_at.unwrap_or(now);
    recording.started_at <= run_ended_at && recording_ended_at >= run_started_at
}

fn workflow_run_recording_resource(
    recording: StoredSessionRecording,
) -> WorkflowRunRecordingResource {
    WorkflowRunRecordingResource {
        id: recording.id,
        session_id: recording.session_id,
        state: recording.state.as_str().to_string(),
        format: recording.format.as_str().to_string(),
        mime_type: recording.mime_type,
        bytes: recording.bytes,
        duration_ms: recording.duration_ms,
        error: recording.error,
        termination_reason: recording
            .termination_reason
            .map(|reason| reason.as_str().to_string()),
        previous_recording_id: recording.previous_recording_id,
        started_at: recording.started_at,
        completed_at: recording.completed_at,
        content_path: format!(
            "/api/v1/sessions/{}/recordings/{}/content",
            recording.session_id, recording.id
        ),
        created_at: recording.created_at,
        updated_at: recording.updated_at,
    }
}

fn workflow_run_retention_resource(
    state: &ApiState,
    run: &StoredWorkflowRun,
) -> WorkflowRunRetentionResource {
    let output_expire_at = run.completed_at.and_then(|completed_at| {
        state
            .workflow_output_retention
            .map(|retention| completed_at + retention)
    });
    let logs_expire_at = run.completed_at.and_then(|completed_at| {
        state
            .workflow_log_retention
            .map(|retention| completed_at + retention)
    });
    WorkflowRunRetentionResource {
        logs_expire_at,
        output_expire_at,
    }
}

fn recording_mime_type(format: SessionRecordingFormat) -> &'static str {
    match format {
        SessionRecordingFormat::Webm => "video/webm",
    }
}

#[cfg(test)]
mod tests;
