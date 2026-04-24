use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tracing::info;
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
    PersistWorkflowDefinitionRequest, PersistWorkflowDefinitionVersionRequest,
    PersistWorkflowRunEventRequest, PersistWorkflowRunLogRequest, PersistWorkflowRunRequest,
    WorkflowDefinitionListResponse, WorkflowDefinitionResource, WorkflowDefinitionVersionResource,
    WorkflowRunEventListResponse, WorkflowRunEventResource, WorkflowRunLogListResponse,
    WorkflowRunLogResource, WorkflowRunResource, WorkflowRunState, WorkflowRunTransitionRequest,
};
use crate::workflow_source::{WorkflowSource, WorkflowSourceError, WorkflowSourceResolver};
use crate::workspace_file_store::{
    StoreWorkspaceFileRequest, WorkspaceFileStore, WorkspaceFileStoreError,
};

/// Shared state for the HTTP API.
struct ApiState {
    registry: Arc<SessionRegistry>,
    auth_validator: Arc<AuthValidator>,
    connect_ticket_manager: Arc<SessionConnectTicketManager>,
    automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
    recording_artifact_store: Arc<RecordingArtifactStore>,
    workspace_file_store: Arc<WorkspaceFileStore>,
    workflow_source_resolver: Arc<WorkflowSourceResolver>,
    recording_observability: Arc<RecordingObservability>,
    recording_lifecycle: Arc<RecordingLifecycleManager>,
    idle_stop_timeout: std::time::Duration,
    public_gateway_url: String,
    default_owner_mode: SessionOwnerMode,
}

const AUTOMATION_ACCESS_TOKEN_HEADER: &str = "x-bpane-automation-access-token";
const FILE_WORKSPACE_FILE_NAME_HEADER: &str = "x-bpane-file-name";
const FILE_WORKSPACE_FILE_PROVENANCE_HEADER: &str = "x-bpane-file-provenance";

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

#[derive(Deserialize)]
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

#[derive(Deserialize)]
struct CreateWorkflowRunRequest {
    workflow_id: Uuid,
    version: String,
    #[serde(default)]
    session: Option<AutomationTaskSessionRequest>,
    #[serde(default)]
    input: Option<Value>,
    #[serde(default)]
    labels: std::collections::HashMap<String, String>,
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
struct AppendWorkflowRunLogRequest {
    stream: AutomationTaskLogStream,
    message: String,
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

async fn create_session(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateSessionRequest>,
) -> Result<(StatusCode, Json<SessionResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let owner_mode = resolve_owner_mode(&state, request.owner_mode)?;
    let stored = create_owned_session(&state, &principal, request, owner_mode).await?;

    Ok((
        StatusCode::CREATED,
        Json(session_resource(&state, &stored, None)),
    ))
}

async fn list_sessions(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let sessions = state
        .session_store
        .list_sessions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|session| session_resource(&state, &session, None))
        .collect();

    Ok(Json(SessionListResponse { sessions }))
}

async fn get_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let stored = authorize_visible_session_request(&headers, &state, session_id).await?;

    Ok(Json(session_resource(&state, &stored, None)))
}

async fn list_automation_tasks(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let tasks = state
        .session_store
        .list_automation_tasks_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|task| task.to_resource())
        .collect();
    Ok(Json(AutomationTaskListResponse { tasks }))
}

async fn create_automation_task(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateAutomationTaskRequest>,
) -> Result<(StatusCode, Json<AutomationTaskResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let (session_id, session_source) =
        resolve_task_session_binding(&state, &principal, Some(request.session), None).await?;

    let task = state
        .session_store
        .create_automation_task(
            &principal,
            PersistAutomationTaskRequest {
                display_name: request.display_name,
                executor: request.executor,
                session_id,
                session_source,
                input: request.input,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;

    Ok((StatusCode::CREATED, Json(task.to_resource())))
}

async fn get_automation_task(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskResource>, (StatusCode, Json<ErrorResponse>)> {
    let task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    Ok(Json(task.to_resource()))
}

async fn cancel_automation_task(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let task = state
        .session_store
        .cancel_automation_task_for_owner(&principal, task_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("automation task {task_id} not found"),
                }),
            )
        })?;
    Ok(Json(task.to_resource()))
}

async fn transition_automation_task_state(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TransitionAutomationTaskRequest>,
) -> Result<Json<AutomationTaskResource>, (StatusCode, Json<ErrorResponse>)> {
    let _task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    let message = request.message.unwrap_or_else(|| match request.state {
        AutomationTaskState::Pending => "automation task returned to pending state".to_string(),
        AutomationTaskState::Starting => "automation task started".to_string(),
        AutomationTaskState::Running => "automation task entered running state".to_string(),
        AutomationTaskState::AwaitingInput => "automation task is awaiting input".to_string(),
        AutomationTaskState::Succeeded => "automation task completed successfully".to_string(),
        AutomationTaskState::Failed => "automation task failed".to_string(),
        AutomationTaskState::Cancelled => "automation task cancelled".to_string(),
        AutomationTaskState::TimedOut => "automation task timed out".to_string(),
    });
    let event_type = match request.state {
        AutomationTaskState::Pending => "automation_task.pending",
        AutomationTaskState::Starting => "automation_task.starting",
        AutomationTaskState::Running => "automation_task.running",
        AutomationTaskState::AwaitingInput => "automation_task.awaiting_input",
        AutomationTaskState::Succeeded => "automation_task.succeeded",
        AutomationTaskState::Failed => "automation_task.failed",
        AutomationTaskState::Cancelled => "automation_task.cancelled",
        AutomationTaskState::TimedOut => "automation_task.timed_out",
    };
    let task = state
        .session_store
        .transition_automation_task(
            task_id,
            AutomationTaskTransitionRequest {
                state: request.state,
                output: request.output,
                error: request.error,
                artifact_refs: request.artifact_refs,
                event_type: event_type.to_string(),
                event_message: message,
                event_data: request.data,
            },
        )
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("automation task {task_id} not found"),
                }),
            )
        })?;
    Ok(Json(task.to_resource()))
}

async fn append_automation_task_log(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<AppendAutomationTaskLogRequest>,
) -> Result<
    Json<crate::automation_task::AutomationTaskLogLineResource>,
    (StatusCode, Json<ErrorResponse>),
> {
    let _task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    let log = state
        .session_store
        .append_automation_task_log(task_id, request.stream, request.message)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("automation task {task_id} not found"),
                }),
            )
        })?;
    Ok(Json(log.to_resource()))
}

async fn get_automation_task_events(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskEventListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    let principal = load_session_owner_principal(&state, task.session_id).await?;
    let events = state
        .session_store
        .list_automation_task_events_for_owner(&principal, task_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|event| event.to_resource())
        .collect();
    Ok(Json(AutomationTaskEventListResponse { events }))
}

async fn get_automation_task_logs(
    headers: HeaderMap,
    Path(task_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<AutomationTaskLogListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let task =
        authorize_visible_automation_task_request_with_automation_access(&headers, &state, task_id)
            .await?;
    let principal = load_session_owner_principal(&state, task.session_id).await?;
    let logs = state
        .session_store
        .list_automation_task_logs_for_owner(&principal, task_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|log| log.to_resource())
        .collect();
    Ok(Json(AutomationTaskLogListResponse { logs }))
}

async fn list_workflow_definitions(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowDefinitionListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workflows = state
        .session_store
        .list_workflow_definitions_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|workflow| workflow.to_resource())
        .collect();
    Ok(Json(WorkflowDefinitionListResponse { workflows }))
}

async fn create_workflow_definition(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateWorkflowDefinitionRequest>,
) -> Result<(StatusCode, Json<WorkflowDefinitionResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workflow = state
        .session_store
        .create_workflow_definition(
            &principal,
            PersistWorkflowDefinitionRequest {
                name: request.name,
                description: request.description,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(workflow.to_resource())))
}

async fn get_workflow_definition(
    headers: HeaderMap,
    Path(workflow_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowDefinitionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workflow = state
        .session_store
        .get_workflow_definition_for_owner(&principal, workflow_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow definition {workflow_id} not found"),
                }),
            )
        })?;
    Ok(Json(workflow.to_resource()))
}

async fn create_workflow_definition_version(
    headers: HeaderMap,
    Path(workflow_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateWorkflowDefinitionVersionRequest>,
) -> Result<(StatusCode, Json<WorkflowDefinitionVersionResource>), (StatusCode, Json<ErrorResponse>)>
{
    let CreateWorkflowDefinitionVersionRequest {
        version,
        executor,
        entrypoint,
        source,
        input_schema,
        output_schema,
        default_session,
        allowed_credential_binding_ids,
        allowed_extension_ids,
        allowed_file_workspace_ids,
    } = request;
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let resolved_source = state
        .workflow_source_resolver
        .resolve(source)
        .await
        .map_err(map_workflow_source_error)?;
    let version = state
        .session_store
        .create_workflow_definition_version(
            &principal,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: workflow_id,
                version,
                executor,
                entrypoint,
                source: resolved_source,
                input_schema,
                output_schema,
                default_session,
                allowed_credential_binding_ids,
                allowed_extension_ids,
                allowed_file_workspace_ids,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(version.to_resource())))
}

async fn get_workflow_definition_version(
    headers: HeaderMap,
    Path((workflow_id, version)): Path<(Uuid, String)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowDefinitionVersionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let version_resource = state
        .session_store
        .get_workflow_definition_version_for_owner(&principal, workflow_id, &version)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "workflow definition version {version} for workflow {workflow_id} not found"
                    ),
                }),
            )
        })?;
    Ok(Json(version_resource.to_resource()))
}

async fn create_workflow_run(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateWorkflowRunRequest>,
) -> Result<(StatusCode, Json<WorkflowRunResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workflow = state
        .session_store
        .get_workflow_definition_for_owner(&principal, request.workflow_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("workflow definition {} not found", request.workflow_id),
                }),
            )
        })?;
    let version = state
        .session_store
        .get_workflow_definition_version_for_owner(
            &principal,
            request.workflow_id,
            &request.version,
        )
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "workflow definition version {} for workflow {} not found",
                        request.version, request.workflow_id
                    ),
                }),
            )
        })?;
    let (session_id, session_source) = resolve_task_session_binding(
        &state,
        &principal,
        request.session,
        version.default_session.as_ref(),
    )
    .await?;
    let task = state
        .session_store
        .create_automation_task(
            &principal,
            PersistAutomationTaskRequest {
                display_name: Some(format!("{} {}", workflow.name, version.version)),
                executor: version.executor.clone(),
                session_id,
                session_source,
                input: request.input.clone(),
                labels: request.labels.clone(),
            },
        )
        .await
        .map_err(map_session_store_error)?;
    let run = state
        .session_store
        .create_workflow_run(
            &principal,
            PersistWorkflowRunRequest {
                workflow_definition_id: workflow.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                session_id,
                automation_task_id: task.id,
                input: request.input,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(run.to_resource())))
}

async fn get_workflow_run(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    Ok(Json(run.to_resource()))
}

async fn cancel_workflow_run(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let run = state
        .session_store
        .get_workflow_run_for_owner(&principal, run_id)
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
    let _task = state
        .session_store
        .cancel_automation_task_for_owner(&principal, run.automation_task_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "automation task {} for workflow run {run_id} not found",
                        run.automation_task_id
                    ),
                }),
            )
        })?;
    let _ = state
        .session_store
        .append_workflow_run_event_for_owner(
            &principal,
            run.id,
            PersistWorkflowRunEventRequest {
                event_type: "workflow_run.cancel_requested".to_string(),
                message: "workflow run cancellation requested".to_string(),
                data: Some(serde_json::json!({
                    "automation_task_id": run.automation_task_id,
                })),
            },
        )
        .await
        .map_err(map_session_store_error)?;
    let run = state
        .session_store
        .get_workflow_run_for_owner(&principal, run_id)
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
    Ok(Json(run.to_resource()))
}

async fn transition_workflow_run_state(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<TransitionWorkflowRunRequest>,
) -> Result<Json<WorkflowRunResource>, (StatusCode, Json<ErrorResponse>)> {
    let _run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let run = state
        .session_store
        .transition_workflow_run(
            run_id,
            WorkflowRunTransitionRequest {
                state: request.state,
                output: request.output,
                error: request.error,
                artifact_refs: request.artifact_refs,
                message: request.message,
                data: request.data,
            },
        )
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
    Ok(Json(run.to_resource()))
}

async fn append_workflow_run_log(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<AppendWorkflowRunLogRequest>,
) -> Result<Json<WorkflowRunLogResource>, (StatusCode, Json<ErrorResponse>)> {
    let _run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let log = state
        .session_store
        .append_workflow_run_log(
            run_id,
            PersistWorkflowRunLogRequest {
                stream: request.stream,
                message: request.message,
            },
        )
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
    Ok(Json(WorkflowRunLogResource::from_run(run_id, &log)))
}

async fn get_workflow_run_events(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunEventListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let principal = load_session_owner_principal(&state, run.session_id).await?;
    let mut events = state
        .session_store
        .list_workflow_run_events_for_owner(&principal, run_id)
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
    Ok(Json(WorkflowRunEventListResponse { events }))
}

async fn get_workflow_run_logs(
    headers: HeaderMap,
    Path(run_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<WorkflowRunLogListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let run =
        authorize_visible_workflow_run_request_with_automation_access(&headers, &state, run_id)
            .await?;
    let principal = load_session_owner_principal(&state, run.session_id).await?;
    let mut logs = state
        .session_store
        .list_workflow_run_logs_for_owner(&principal, run_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|log| WorkflowRunLogResource::from_run(run.id, &log))
        .collect::<Vec<_>>();
    let task_logs = state
        .session_store
        .list_automation_task_logs_for_owner(&principal, run.automation_task_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|log| {
            WorkflowRunLogResource::from_automation_task(run.id, run.automation_task_id, &log)
        });
    logs.extend(task_logs);
    logs.sort_by(|left, right| {
        left.created_at
            .cmp(&right.created_at)
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(Json(WorkflowRunLogListResponse { logs }))
}

async fn list_file_workspaces(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workspaces = state
        .session_store
        .list_file_workspaces_for_owner(&principal)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|workspace| workspace.to_resource())
        .collect();
    Ok(Json(FileWorkspaceListResponse { workspaces }))
}

async fn create_file_workspace(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CreateFileWorkspaceRequest>,
) -> Result<(StatusCode, Json<FileWorkspaceResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let workspace = state
        .session_store
        .create_file_workspace(
            &principal,
            PersistFileWorkspaceRequest {
                name: request.name,
                description: request.description,
                labels: request.labels,
            },
        )
        .await
        .map_err(map_session_store_error)?;
    Ok((StatusCode::CREATED, Json(workspace.to_resource())))
}

async fn get_file_workspace(
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceResource>, (StatusCode, Json<ErrorResponse>)> {
    let workspace = authorize_file_workspace_request(&headers, &state, workspace_id).await?;
    Ok(Json(workspace.to_resource()))
}

async fn list_file_workspace_files(
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceFileListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let _workspace = state
        .session_store
        .get_file_workspace_for_owner(&principal, workspace_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("file workspace {workspace_id} not found"),
                }),
            )
        })?;
    let files = state
        .session_store
        .list_file_workspace_files_for_owner(&principal, workspace_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|file| file.to_resource())
        .collect();
    Ok(Json(FileWorkspaceFileListResponse { files }))
}

async fn upload_file_workspace_file(
    headers: HeaderMap,
    Path(workspace_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    body: Bytes,
) -> Result<(StatusCode, Json<FileWorkspaceFileResource>), (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let _workspace = state
        .session_store
        .get_file_workspace_for_owner(&principal, workspace_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("file workspace {workspace_id} not found"),
                }),
            )
        })?;
    let file_name = required_header_string(&headers, FILE_WORKSPACE_FILE_NAME_HEADER)?;
    let provenance =
        parse_optional_json_object_header(&headers, FILE_WORKSPACE_FILE_PROVENANCE_HEADER)?;
    let media_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned);
    let file_id = Uuid::now_v7();
    let sha256_hex = hex::encode(Sha256::digest(body.as_ref()));
    let stored_artifact = state
        .workspace_file_store
        .write(StoreWorkspaceFileRequest {
            workspace_id,
            file_id,
            file_name: file_name.clone(),
            bytes: body.to_vec(),
        })
        .await
        .map_err(map_workspace_file_store_error)?;
    let persisted = state
        .session_store
        .create_file_workspace_file_for_owner(
            &principal,
            PersistFileWorkspaceFileRequest {
                id: file_id,
                workspace_id,
                name: file_name,
                media_type,
                byte_count: body.len() as u64,
                sha256_hex,
                provenance,
                artifact_ref: stored_artifact.artifact_ref.clone(),
            },
        )
        .await;
    let persisted = match persisted {
        Ok(file) => file,
        Err(error) => {
            let _ = state
                .workspace_file_store
                .delete(&stored_artifact.artifact_ref)
                .await;
            return Err(map_session_store_error(error));
        }
    };
    Ok((StatusCode::CREATED, Json(persisted.to_resource())))
}

async fn get_file_workspace_file(
    headers: HeaderMap,
    Path((workspace_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceFileResource>, (StatusCode, Json<ErrorResponse>)> {
    let file =
        authorize_file_workspace_file_request(&headers, &state, workspace_id, file_id).await?;
    Ok(Json(file.to_resource()))
}

async fn get_file_workspace_file_content(
    headers: HeaderMap,
    Path((workspace_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let file =
        authorize_file_workspace_file_request(&headers, &state, workspace_id, file_id).await?;
    let bytes = state
        .workspace_file_store
        .read(&file.artifact_ref)
        .await
        .map_err(map_workspace_file_content_error)?;
    let media_type = file
        .media_type
        .clone()
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response.headers_mut().insert(
        CONTENT_TYPE,
        header_value_or_default(&media_type, "application/octet-stream"),
    );
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string())
            .unwrap_or_else(|_| HeaderValue::from_static("0")),
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        header_value_or_default(
            &format!(
                "attachment; filename=\"{}\"",
                sanitize_content_disposition_filename(&file.name)
            ),
            "attachment",
        ),
    );
    Ok(response)
}

async fn delete_file_workspace_file(
    headers: HeaderMap,
    Path((workspace_id, file_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<FileWorkspaceFileResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let file = state
        .session_store
        .get_file_workspace_file_for_owner(&principal, workspace_id, file_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "file workspace file {file_id} for workspace {workspace_id} not found"
                    ),
                }),
            )
        })?;
    state
        .workspace_file_store
        .delete(&file.artifact_ref)
        .await
        .or_else(|error| match error {
            WorkspaceFileStoreError::Backend(inner)
                if inner.kind() == std::io::ErrorKind::NotFound =>
            {
                Ok(())
            }
            other => Err(other),
        })
        .map_err(map_workspace_file_content_error)?;
    let deleted = state
        .session_store
        .delete_file_workspace_file_for_owner(&principal, workspace_id, file_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "file workspace file {file_id} for workspace {workspace_id} not found"
                    ),
                }),
            )
        })?;
    Ok(Json(deleted.to_resource()))
}

async fn list_session_recordings(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingListResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?
        .into_iter()
        .map(|recording| recording.to_resource())
        .collect();

    Ok(Json(SessionRecordingListResponse { recordings }))
}

async fn create_session_recording(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<(StatusCode, Json<SessionRecordingResource>), (StatusCode, Json<ErrorResponse>)> {
    let session = authorize_runtime_session_request(&headers, &state, session_id).await?;
    if session.recording.mode == SessionRecordingMode::Disabled {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("recording is disabled for session {session_id}"),
            }),
        ));
    }

    let recording = state
        .session_store
        .create_recording_for_session(session_id, session.recording.format, None)
        .await
        .map_err(map_session_store_error)?;

    Ok((StatusCode::CREATED, Json(recording.to_resource())))
}

async fn get_session_recording(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = load_session_recording(&state, session_id, recording_id).await?;
    Ok(Json(recording.to_resource()))
}

async fn stop_session_recording(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_runtime_session_request(&headers, &state, session_id).await?;
    let recording = state
        .session_store
        .stop_recording_for_session(
            session_id,
            recording_id,
            SessionRecordingTerminationReason::ManualStop,
        )
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
        })?;
    Ok(Json(recording.to_resource()))
}

async fn complete_session_recording(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<CompleteSessionRecordingRequest>,
) -> Result<Json<SessionRecordingResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = load_session_recording(&state, session_id, recording_id).await?;
    let CompleteSessionRecordingRequest {
        source_path,
        mime_type,
        bytes,
        duration_ms,
    } = request;
    state
        .recording_observability
        .record_artifact_finalize_request();
    let stored_artifact = state
        .recording_artifact_store
        .finalize(FinalizeRecordingArtifactRequest {
            session_id,
            recording_id,
            format: recording.format,
            source_path,
        })
        .await
        .map_err(|error| {
            state
                .recording_observability
                .record_artifact_finalize_failure();
            map_recording_artifact_store_error(error)
        })?;
    let recording = state
        .session_store
        .complete_recording_for_session(
            session_id,
            recording_id,
            PersistCompletedSessionRecordingRequest {
                artifact_ref: stored_artifact.artifact_ref.clone(),
                mime_type,
                bytes,
                duration_ms,
            },
        )
        .await
        .map_err(|error| {
            let artifact_store = state.recording_artifact_store.clone();
            let artifact_ref = stored_artifact.artifact_ref.clone();
            tokio::spawn(async move {
                let _ = artifact_store.delete(&artifact_ref).await;
            });
            state
                .recording_observability
                .record_artifact_finalize_failure();
            map_session_store_error(error)
        })?
        .ok_or_else(|| {
            let artifact_store = state.recording_artifact_store.clone();
            let artifact_ref = stored_artifact.artifact_ref.clone();
            tokio::spawn(async move {
                let _ = artifact_store.delete(&artifact_ref).await;
            });
            state
                .recording_observability
                .record_artifact_finalize_failure();
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "recording {recording_id} was not found for session {session_id}"
                    ),
                }),
            )
        })?;
    state
        .recording_observability
        .record_artifact_finalize_success();
    Ok(Json(recording.to_resource()))
}

async fn fail_session_recording(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<FailSessionRecordingRequest>,
) -> Result<Json<SessionRecordingResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = state
        .session_store
        .fail_recording_for_session(session_id, recording_id, request)
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
        })?;
    state.recording_observability.record_recording_failure();
    Ok(Json(recording.to_resource()))
}

async fn get_session_recording_content(
    headers: HeaderMap,
    Path((session_id, recording_id)): Path<(Uuid, Uuid)>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let recording = load_session_recording(&state, session_id, recording_id).await?;
    let artifact_ref = recording.artifact_ref.as_ref().ok_or_else(|| {
        if recording.state.is_terminal() {
            (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: format!("recording artifact for {recording_id} is no longer available"),
                }),
            )
        } else {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("recording {recording_id} does not have an artifact yet"),
                }),
            )
        }
    })?;
    let bytes = state
        .recording_artifact_store
        .read(artifact_ref)
        .await
        .map_err(|error| match error.io_kind() {
            Some(std::io::ErrorKind::NotFound) => (
                StatusCode::GONE,
                Json(ErrorResponse {
                    error: format!("recording artifact for {recording_id} is no longer available"),
                }),
            ),
            _ => (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to read recording artifact: {error}"),
                }),
            ),
        })?;

    let filename = format!("browserpane-{session_id}-{recording_id}.webm");
    let mime_type = recording
        .mime_type
        .as_deref()
        .unwrap_or(recording_mime_type(recording.format));

    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_str(mime_type).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to encode content type header: {error}"),
                }),
            )
        })?,
    );
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string()).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to encode content length header: {error}"),
                }),
            )
        })?,
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).map_err(
            |error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to encode content disposition header: {error}"),
                    }),
                )
            },
        )?,
    );
    Ok(response)
}

async fn get_session_recording_playback(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingPlaybackResource>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    let playback = load_session_recording_playback(&state, session_id).await?;
    Ok(Json(playback.resource))
}

async fn get_session_recording_playback_manifest(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionRecordingPlaybackManifest>, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    state
        .recording_observability
        .record_playback_manifest_request();
    let playback = load_session_recording_playback(&state, session_id).await?;
    Ok(Json(playback.manifest))
}

async fn get_session_recording_playback_export(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    let _session = authorize_visible_session_request(&headers, &state, session_id).await?;
    state
        .recording_observability
        .record_playback_export_request();
    let playback = load_session_recording_playback(&state, session_id).await?;
    let bytes = playback
        .export_bundle(&state.recording_artifact_store)
        .await
        .map_err(|error| {
            state
                .recording_observability
                .record_playback_export_failure();
            map_recording_playback_error(error)
        })?;
    state
        .recording_observability
        .record_playback_export_success(bytes.len() as u64, Utc::now())
        .await;

    let filename = format!("browserpane-{session_id}-recording-playback.zip");
    let mut response = Response::new(axum::body::Body::from(bytes.clone()));
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/zip"));
    response.headers_mut().insert(
        CONTENT_LENGTH,
        HeaderValue::from_str(&bytes.len().to_string()).map_err(|error| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("failed to encode content length header: {error}"),
                }),
            )
        })?,
    );
    response.headers_mut().insert(
        CONTENT_DISPOSITION,
        HeaderValue::from_str(&format!("attachment; filename=\"{filename}\"")).map_err(
            |error| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("failed to encode content disposition header: {error}"),
                    }),
                )
            },
        )?,
    );
    Ok(response)
}

async fn get_recording_operations(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<RecordingObservabilitySnapshot>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    Ok(Json(state.recording_observability.snapshot().await))
}

async fn set_automation_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(request): Json<SetAutomationDelegateRequest>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let stored = state
        .session_store
        .set_automation_delegate_for_owner(&principal, session_id, request)
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

    Ok(Json(session_resource(&state, &stored, None)))
}

async fn clear_automation_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let stored = state
        .session_store
        .clear_automation_delegate_for_owner(&principal, session_id)
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

    Ok(Json(session_resource(&state, &stored, None)))
}

async fn issue_session_access_token(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionAccessTokenResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let connectable = prepare_runtime_access_session(&state, &principal, session_id).await?;

    let issued = state
        .connect_ticket_manager
        .issue_ticket(session_id, &principal)
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to issue session connect ticket: {error}"),
                }),
            )
        })?;
    let resource = session_resource(&state, &connectable, None);

    Ok(Json(SessionAccessTokenResponse {
        session_id,
        token_type: "session_connect_ticket".to_string(),
        token: issued.token,
        expires_at: issued.expires_at,
        connect: resource.connect,
    }))
}

async fn issue_session_automation_access(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionAutomationAccessResponse>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let connectable = prepare_runtime_access_session(&state, &principal, session_id).await?;
    resolve_runtime(&state, session_id).await?;
    let resource = session_resource(&state, &connectable, None);
    let endpoint_url = resource.runtime.cdp_endpoint.ok_or_else(|| {
        (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} does not expose an automation endpoint for the current runtime"
                ),
            }),
        )
    })?;
    let issued = state
        .automation_access_token_manager
        .issue_token(session_id, &principal)
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to issue session automation access token: {error}"),
                }),
            )
        })?;

    Ok(Json(SessionAutomationAccessResponse {
        session_id,
        token_type: "session_automation_access_token".to_string(),
        token: issued.token,
        expires_at: issued.expires_at,
        automation: SessionAutomationAccessInfo {
            endpoint_url,
            protocol: "chrome_devtools_protocol".to_string(),
            auth_type: "session_automation_access_token".to_string(),
            auth_header: AUTOMATION_ACCESS_TOKEN_HEADER.to_string(),
            status_path: format!("/api/v1/sessions/{session_id}/status"),
            mcp_owner_path: format!("/api/v1/sessions/{session_id}/mcp-owner"),
            compatibility_mode: resource.connect.compatibility_mode,
        },
    }))
}

async fn get_session_status(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    let session =
        authorize_runtime_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let hub = state
        .registry
        .ensure_hub_for_session(
            session_id,
            &resolve_runtime(&state, session_id).await?.agent_socket_path,
        )
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;
    let snapshot = hub.telemetry_snapshot().await;
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    let latest_recording = latest_recording(&recordings);
    let playback = prepare_session_recording_playback(session_id, &recordings, Utc::now());

    Ok(Json(session_status_from_snapshot(
        snapshot,
        &session.recording,
        latest_recording,
        playback.resource,
    )))
}

async fn delete_session(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionResource>, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;

    let stored = state
        .session_store
        .get_session_for_owner(&principal, session_id)
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

    if should_block_session_stop(
        stored.state,
        state
            .session_manager
            .profile()
            .supports_legacy_global_routes,
        runtime_is_currently_in_use(&state).await,
    ) {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: "cannot stop the legacy single-session runtime while it is in use"
                    .to_string(),
            }),
        ));
    }

    let stopped = state
        .session_store
        .stop_session_for_owner(&principal, session_id)
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

    if let Err(error) = state
        .recording_lifecycle
        .request_stop_and_wait(session_id, SessionRecordingTerminationReason::SessionStop)
        .await
    {
        info!(%session_id, "recording finalization before session stop returned: {error}");
    }
    state.session_manager.release(session_id).await;
    state.registry.remove_session(session_id).await;

    Ok(Json(session_resource(&state, &stopped, None)))
}

/// GET /api/session/status
async fn session_status(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<SessionStatus>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    ensure_legacy_runtime_routes_supported(&state)?;
    let Some(session_id) = legacy_runtime_session_id(&state).await else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "no runtime-backed session is available".to_string(),
            }),
        ));
    };
    let runtime = resolve_runtime_compat(&state, session_id)
        .await
        .map_err(map_runtime_compat_status)?;
    let session = state
        .session_store
        .get_session_by_id(session_id)
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
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;
    let snapshot = hub.telemetry_snapshot().await;
    let recordings = state
        .session_store
        .list_recordings_for_session(session_id)
        .await
        .map_err(map_session_store_error)?;
    let latest_recording = latest_recording(&recordings);
    let playback = prepare_session_recording_playback(session_id, &recordings, Utc::now());

    Ok(Json(session_status_from_snapshot(
        snapshot,
        &session.recording,
        latest_recording,
        playback.resource,
    )))
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

/// POST /api/session/mcp-owner
async fn set_mcp_owner(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<McpOwnerRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    ensure_legacy_runtime_routes_supported(&state)?;
    let Some(session_id) = legacy_runtime_session_id(&state).await else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: "no runtime-backed session is available".to_string(),
            }),
        ));
    };
    let runtime = resolve_runtime(&state, session_id).await?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|e| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {e}"),
                }),
            )
        })?;

    hub.set_mcp_owner(req.width, req.height).await;
    state.session_manager.mark_session_active(session_id).await;
    let _ = state.session_store.mark_session_active(session_id).await;

    Ok(Json(OkResponse { ok: true }))
}

async fn set_session_mcp_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
    Json(req): Json<McpOwnerRequest>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_runtime_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let runtime = resolve_runtime(&state, session_id).await?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;

    hub.set_mcp_owner(req.width, req.height).await;
    state.session_manager.mark_session_active(session_id).await;
    let _ = state.session_store.mark_session_active(session_id).await;

    Ok(Json(OkResponse { ok: true }))
}

/// DELETE /api/session/mcp-owner
async fn clear_mcp_owner(
    headers: HeaderMap,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<OkResponse>, StatusCode> {
    authorize_api_request(&headers, &state.auth_validator)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    if !state
        .session_manager
        .profile()
        .supports_legacy_global_routes
    {
        return Err(StatusCode::CONFLICT);
    }
    let Some(session_id) = legacy_runtime_session_id(&state).await else {
        return Err(StatusCode::NOT_FOUND);
    };
    let runtime = resolve_runtime_compat(&state, session_id)
        .await
        .map_err(|status| status)?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;

    hub.clear_mcp_owner().await;
    let snapshot = hub.telemetry_snapshot().await;
    if snapshot.browser_clients == 0 && snapshot.viewer_clients == 0 && !snapshot.mcp_owner {
        let _ = state.session_store.mark_session_idle(session_id).await;
        state.session_manager.mark_session_idle(session_id).await;
        schedule_idle_session_stop(
            session_id,
            state.idle_stop_timeout,
            state.registry.clone(),
            state.session_store.clone(),
            state.session_manager.clone(),
            state.recording_lifecycle.clone(),
        );
    }

    Ok(Json(OkResponse { ok: true }))
}

async fn clear_session_mcp_owner(
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
    State(state): State<Arc<ApiState>>,
) -> Result<Json<OkResponse>, (StatusCode, Json<ErrorResponse>)> {
    let _session =
        authorize_runtime_session_request_with_automation_access(&headers, &state, session_id)
            .await?;
    let runtime = resolve_runtime(&state, session_id).await?;
    let hub = state
        .registry
        .ensure_hub_for_session(session_id, &runtime.agent_socket_path)
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: format!("failed to connect to host agent: {error}"),
                }),
            )
        })?;

    hub.clear_mcp_owner().await;
    let snapshot = hub.telemetry_snapshot().await;
    if snapshot.browser_clients == 0 && snapshot.viewer_clients == 0 && !snapshot.mcp_owner {
        let _ = state.session_store.mark_session_idle(session_id).await;
        state.session_manager.mark_session_idle(session_id).await;
        schedule_idle_session_stop(
            session_id,
            state.idle_stop_timeout,
            state.registry.clone(),
            state.session_store.clone(),
            state.session_manager.clone(),
            state.recording_lifecycle.clone(),
        );
    }

    Ok(Json(OkResponse { ok: true }))
}

/// Runs the HTTP API server for MCP bridge communication.
pub async fn run_api_server(
    bind_addr: SocketAddr,
    registry: Arc<SessionRegistry>,
    auth_validator: Arc<AuthValidator>,
    connect_ticket_manager: Arc<SessionConnectTicketManager>,
    automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    session_store: SessionStore,
    session_manager: Arc<SessionManager>,
    recording_artifact_store: Arc<RecordingArtifactStore>,
    workspace_file_store: Arc<WorkspaceFileStore>,
    workflow_source_resolver: Arc<WorkflowSourceResolver>,
    recording_observability: Arc<RecordingObservability>,
    recording_lifecycle: Arc<RecordingLifecycleManager>,
    idle_stop_timeout: std::time::Duration,
    public_gateway_url: String,
    default_owner_mode: SessionOwnerMode,
) -> anyhow::Result<()> {
    let state = Arc::new(ApiState {
        registry,
        auth_validator,
        connect_ticket_manager,
        automation_access_token_manager,
        session_store,
        session_manager,
        recording_artifact_store,
        workspace_file_store,
        workflow_source_resolver,
        recording_observability,
        recording_lifecycle,
        idle_stop_timeout,
        public_gateway_url,
        default_owner_mode,
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
    request: CreateSessionRequest,
    owner_mode: SessionOwnerMode,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
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
) -> Result<(Uuid, AutomationTaskSessionSource), (StatusCode, Json<ErrorResponse>)> {
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
            Ok((visible.id, AutomationTaskSessionSource::ExistingSession))
        }
        Some(AutomationTaskSessionRequest {
            existing_session_id: None,
            create_session: Some(create_session_request),
        }) => {
            let owner_mode = resolve_owner_mode(state, create_session_request.owner_mode)?;
            let created =
                create_owned_session(state, principal, create_session_request, owner_mode).await?;
            Ok((created.id, AutomationTaskSessionSource::CreatedSession))
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
            let created =
                create_owned_session(state, principal, create_session_request, owner_mode).await?;
            Ok((created.id, AutomationTaskSessionSource::CreatedSession))
        }
    }
}

async fn load_session_owner_principal(
    state: &ApiState,
    session_id: Uuid,
) -> Result<AuthenticatedPrincipal, (StatusCode, Json<ErrorResponse>)> {
    let session = state
        .session_store
        .get_session_by_id(session_id)
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
    Ok(AuthenticatedPrincipal {
        subject: session.owner.subject,
        issuer: session.owner.issuer,
        display_name: session.owner.display_name,
        client_id: None,
    })
}

async fn authorize_visible_automation_task_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    task_id: Uuid,
) -> Result<crate::automation_task::StoredAutomationTask, (StatusCode, Json<ErrorResponse>)> {
    if extract_bearer_token(headers).is_some() {
        match authorize_api_request(headers, &state.auth_validator).await {
            Ok(principal) => {
                if let Some(task) = state
                    .session_store
                    .get_automation_task_for_owner(&principal, task_id)
                    .await
                    .map_err(map_session_store_error)?
                {
                    return Ok(task);
                }
                if extract_automation_access_token(headers).is_none() {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: format!("automation task {task_id} not found"),
                        }),
                    ));
                }
            }
            Err(error) if extract_automation_access_token(headers).is_none() => {
                return Err((StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })));
            }
            Err(_) => {}
        }
    }

    let claims = validate_any_automation_access_request(headers, state)?;
    let task = state
        .session_store
        .get_automation_task_by_id(task_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("automation task {task_id} not found"),
                }),
            )
        })?;
    let session = state
        .session_store
        .get_session_by_id(task.session_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("session {} not found", task.session_id),
                }),
            )
        })?;
    if !automation_access_claims_match_session(&claims, &session) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "automation access token is no longer valid for this session".to_string(),
            }),
        ));
    }
    Ok(task)
}

async fn authorize_visible_workflow_run_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    run_id: Uuid,
) -> Result<crate::workflow::StoredWorkflowRun, (StatusCode, Json<ErrorResponse>)> {
    if extract_bearer_token(headers).is_some() {
        match authorize_api_request(headers, &state.auth_validator).await {
            Ok(principal) => {
                if let Some(run) = state
                    .session_store
                    .get_workflow_run_for_owner(&principal, run_id)
                    .await
                    .map_err(map_session_store_error)?
                {
                    return Ok(run);
                }
                if extract_automation_access_token(headers).is_none() {
                    return Err((
                        StatusCode::NOT_FOUND,
                        Json(ErrorResponse {
                            error: format!("workflow run {run_id} not found"),
                        }),
                    ));
                }
            }
            Err(error) if extract_automation_access_token(headers).is_none() => {
                return Err((StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })));
            }
            Err(_) => {}
        }
    }

    let claims = validate_any_automation_access_request(headers, state)?;
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
    if !automation_access_claims_match_session(&claims, &session) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "automation access token is no longer valid for this session".to_string(),
            }),
        ));
    }
    Ok(run)
}

async fn authorize_file_workspace_request(
    headers: &HeaderMap,
    state: &ApiState,
    workspace_id: Uuid,
) -> Result<crate::file_workspace::StoredFileWorkspace, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    state
        .session_store
        .get_file_workspace_for_owner(&principal, workspace_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("file workspace {workspace_id} not found"),
                }),
            )
        })
}

async fn authorize_file_workspace_file_request(
    headers: &HeaderMap,
    state: &ApiState,
    workspace_id: Uuid,
    file_id: Uuid,
) -> Result<crate::file_workspace::StoredFileWorkspaceFile, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    state
        .session_store
        .get_file_workspace_file_for_owner(&principal, workspace_id, file_id)
        .await
        .map_err(map_session_store_error)?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "file workspace file {file_id} for workspace {workspace_id} not found"
                    ),
                }),
            )
        })
}

async fn authorize_api_request(
    headers: &HeaderMap,
    auth_validator: &AuthValidator,
) -> Result<AuthenticatedPrincipal, String> {
    let token = extract_bearer_token(headers).ok_or_else(|| "missing bearer token".to_string())?;
    auth_validator
        .authenticate(token)
        .await
        .map_err(|error| format!("invalid bearer token: {error}"))
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

fn extract_bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ")
}

fn extract_automation_access_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTOMATION_ACCESS_TOKEN_HEADER)?
        .to_str()
        .ok()
        .map(str::trim)
        .filter(|value| !value.is_empty())
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
        .route("/api/v1/sessions", post(create_session).get(list_sessions))
        .route(
            "/api/v1/file-workspaces",
            post(create_file_workspace).get(list_file_workspaces),
        )
        .route(
            "/api/v1/file-workspaces/{workspace_id}",
            get(get_file_workspace),
        )
        .route(
            "/api/v1/file-workspaces/{workspace_id}/files",
            post(upload_file_workspace_file).get(list_file_workspace_files),
        )
        .route(
            "/api/v1/file-workspaces/{workspace_id}/files/{file_id}",
            get(get_file_workspace_file).delete(delete_file_workspace_file),
        )
        .route(
            "/api/v1/file-workspaces/{workspace_id}/files/{file_id}/content",
            get(get_file_workspace_file_content),
        )
        .route(
            "/api/v1/workflows",
            post(create_workflow_definition).get(list_workflow_definitions),
        )
        .route(
            "/api/v1/workflows/{workflow_id}",
            get(get_workflow_definition),
        )
        .route(
            "/api/v1/workflows/{workflow_id}/versions",
            post(create_workflow_definition_version),
        )
        .route(
            "/api/v1/workflows/{workflow_id}/versions/{version}",
            get(get_workflow_definition_version),
        )
        .route("/api/v1/workflow-runs", post(create_workflow_run))
        .route("/api/v1/workflow-runs/{run_id}", get(get_workflow_run))
        .route(
            "/api/v1/workflow-runs/{run_id}/state",
            post(transition_workflow_run_state),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/cancel",
            post(cancel_workflow_run),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/events",
            get(get_workflow_run_events),
        )
        .route(
            "/api/v1/workflow-runs/{run_id}/logs",
            get(get_workflow_run_logs).post(append_workflow_run_log),
        )
        .route(
            "/api/v1/automation-tasks",
            post(create_automation_task).get(list_automation_tasks),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}",
            get(get_automation_task),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}/state",
            post(transition_automation_task_state),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}/cancel",
            post(cancel_automation_task),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}/events",
            get(get_automation_task_events),
        )
        .route(
            "/api/v1/automation-tasks/{task_id}/logs",
            get(get_automation_task_logs).post(append_automation_task_log),
        )
        .route(
            "/api/v1/sessions/{session_id}",
            get(get_session).delete(delete_session),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings",
            post(create_session_recording).get(list_session_recordings),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}",
            get(get_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/stop",
            post(stop_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/complete",
            post(complete_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/fail",
            post(fail_session_recording),
        )
        .route(
            "/api/v1/sessions/{session_id}/recordings/{recording_id}/content",
            get(get_session_recording_content),
        )
        .route(
            "/api/v1/sessions/{session_id}/recording-playback",
            get(get_session_recording_playback),
        )
        .route(
            "/api/v1/sessions/{session_id}/recording-playback/manifest",
            get(get_session_recording_playback_manifest),
        )
        .route(
            "/api/v1/sessions/{session_id}/recording-playback/export",
            get(get_session_recording_playback_export),
        )
        .route(
            "/api/v1/sessions/{session_id}/access-tokens",
            post(issue_session_access_token),
        )
        .route(
            "/api/v1/sessions/{session_id}/automation-access",
            post(issue_session_automation_access),
        )
        .route(
            "/api/v1/sessions/{session_id}/automation-owner",
            post(set_automation_owner).delete(clear_automation_owner),
        )
        .route(
            "/api/v1/sessions/{session_id}/status",
            get(get_session_status),
        )
        .route(
            "/api/v1/sessions/{session_id}/mcp-owner",
            post(set_session_mcp_owner).delete(clear_session_mcp_owner),
        )
        .route(
            "/api/v1/recording/operations",
            get(get_recording_operations),
        )
        .route("/api/session/status", get(session_status))
        .route("/api/session/mcp-owner", post(set_mcp_owner))
        .route("/api/session/mcp-owner", delete(clear_mcp_owner))
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

fn map_workflow_source_error(error: WorkflowSourceError) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        WorkflowSourceError::Invalid(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        WorkflowSourceError::Resolve(_) => (
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

async fn authorize_runtime_session_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let session = authorize_visible_session_request(headers, state, session_id).await?;

    ensure_runtime_candidate_session(session, session_id)
}

async fn authorize_runtime_session_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let session =
        authorize_visible_session_request_with_automation_access(headers, state, session_id)
            .await?;

    ensure_runtime_candidate_session(session, session_id)
}

fn ensure_runtime_candidate_session(
    session: StoredSession,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    if !matches!(
        session.state,
        SessionLifecycleState::Pending
            | SessionLifecycleState::Starting
            | SessionLifecycleState::Ready
            | SessionLifecycleState::Active
            | SessionLifecycleState::Idle
    ) {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} is not attached to a runtime-compatible state"
                ),
            }),
        ));
    }

    Ok(session)
}

async fn prepare_runtime_access_session(
    state: &ApiState,
    principal: &AuthenticatedPrincipal,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let stored = state
        .session_store
        .get_session_for_principal(principal, session_id)
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
    let was_stopped = stored.state == SessionLifecycleState::Stopped;

    let connectable = if was_stopped {
        let prepared = state
            .session_store
            .prepare_session_for_connect(session_id)
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
        schedule_idle_session_stop(
            session_id,
            state.idle_stop_timeout,
            state.registry.clone(),
            state.session_store.clone(),
            state.session_manager.clone(),
            state.recording_lifecycle.clone(),
        );
        prepared
    } else {
        stored
    };

    if let Err(error) = state
        .recording_lifecycle
        .ensure_auto_recording(&connectable)
        .await
    {
        if was_stopped {
            let _ = state.session_store.stop_session_if_idle(session_id).await;
            state.session_manager.release(session_id).await;
            state.registry.remove_session(session_id).await;
        }
        return Err(map_recording_lifecycle_error(error));
    }

    if !connectable.state.is_runtime_candidate() {
        return Err((
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!(
                    "session {session_id} is not connectable in state {}",
                    connectable.state.as_str()
                ),
            }),
        ));
    }

    Ok(connectable)
}

async fn authorize_visible_session_request_with_automation_access(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    if extract_bearer_token(headers).is_some() {
        match authorize_visible_session_request(headers, state, session_id).await {
            Ok(session) => return Ok(session),
            Err(error) if extract_automation_access_token(headers).is_none() => return Err(error),
            Err(_) => {}
        }
    }

    let claims = validate_automation_access_request(headers, state, session_id)?;
    let session = state
        .session_store
        .get_session_by_id(session_id)
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
    if !automation_access_claims_match_session(&claims, &session) {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "automation access token is no longer valid for this session".to_string(),
            }),
        ));
    }

    Ok(session)
}

fn validate_automation_access_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<SessionAutomationAccessTokenClaims, (StatusCode, Json<ErrorResponse>)> {
    let claims = validate_any_automation_access_request(headers, state)?;
    if claims.session_id != session_id {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "session automation access token does not match the requested session"
                    .to_string(),
            }),
        ));
    }
    Ok(claims)
}

fn validate_any_automation_access_request(
    headers: &HeaderMap,
    state: &ApiState,
) -> Result<SessionAutomationAccessTokenClaims, (StatusCode, Json<ErrorResponse>)> {
    let token = extract_automation_access_token(headers).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "missing bearer token or session automation access token".to_string(),
            }),
        )
    })?;
    state
        .automation_access_token_manager
        .validate_token(token)
        .map_err(|error| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: format!("invalid session automation access token: {error}"),
                }),
            )
        })
}

fn automation_access_claims_match_session(
    claims: &SessionAutomationAccessTokenClaims,
    session: &StoredSession,
) -> bool {
    if session.owner.subject == claims.subject && session.owner.issuer == claims.issuer {
        return true;
    }

    let Some(delegate) = &session.automation_delegate else {
        return false;
    };
    claims.issuer == delegate.issuer
        && claims.client_id.as_deref() == Some(delegate.client_id.as_str())
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

async fn authorize_visible_session_request(
    headers: &HeaderMap,
    state: &ApiState,
    session_id: Uuid,
) -> Result<StoredSession, (StatusCode, Json<ErrorResponse>)> {
    let principal = authorize_api_request(headers, &state.auth_validator)
        .await
        .map_err(|error| (StatusCode::UNAUTHORIZED, Json(ErrorResponse { error })))?;
    let session = state
        .session_store
        .get_session_for_principal(&principal, session_id)
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

    Ok(session)
}

async fn runtime_is_currently_in_use(state: &ApiState) -> bool {
    let Some(session_id) = legacy_runtime_session_id(state).await else {
        return false;
    };
    let Some(snapshot) = state.registry.telemetry_snapshot_if_live(session_id).await else {
        return false;
    };
    snapshot.browser_clients > 0 || snapshot.viewer_clients > 0 || snapshot.mcp_owner
}

fn should_block_session_stop(
    state: SessionLifecycleState,
    supports_legacy_global_routes: bool,
    runtime_in_use: bool,
) -> bool {
    supports_legacy_global_routes && state.is_runtime_candidate() && runtime_in_use
}

fn recording_mime_type(format: SessionRecordingFormat) -> &'static str {
    match format {
        SessionRecordingFormat::Webm => "video/webm",
    }
}

async fn resolve_runtime(
    state: &ApiState,
    session_id: Uuid,
) -> Result<SessionRuntime, (StatusCode, Json<ErrorResponse>)> {
    state
        .session_manager
        .resolve(session_id)
        .await
        .map_err(map_session_manager_error)
}

async fn resolve_runtime_compat(
    state: &ApiState,
    session_id: Uuid,
) -> Result<SessionRuntime, StatusCode> {
    state
        .session_manager
        .resolve(session_id)
        .await
        .map_err(|_| StatusCode::CONFLICT)
}

fn map_session_manager_error(error: SessionManagerError) -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::CONFLICT,
        Json(ErrorResponse {
            error: error.to_string(),
        }),
    )
}

fn map_runtime_compat_status(status: StatusCode) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: if status == StatusCode::CONFLICT {
                "runtime is not currently available for the requested compatibility route"
                    .to_string()
            } else {
                "compatibility route failed".to_string()
            },
        }),
    )
}

fn ensure_legacy_runtime_routes_supported(
    state: &ApiState,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if state
        .session_manager
        .profile()
        .supports_legacy_global_routes
    {
        return Ok(());
    }

    Err((
        StatusCode::CONFLICT,
        Json(ErrorResponse {
            error: "global compatibility routes are disabled for the current runtime backend; use /api/v1/sessions/{id}/status and /api/v1/sessions/{id}/mcp-owner instead".to_string(),
        }),
    ))
}

async fn legacy_runtime_session_id(state: &ApiState) -> Option<Uuid> {
    state
        .session_store
        .get_runtime_candidate_session()
        .await
        .ok()
        .flatten()
        .map(|session| session.id)
}

#[cfg(test)]
mod tests;
