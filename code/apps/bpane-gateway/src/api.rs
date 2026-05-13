use std::collections::HashSet;
use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::http::header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::Response;
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
    PersistCompletedSessionRecordingRequest, PersistSessionFileBindingRequest,
    SessionLifecycleState, SessionListResponse, SessionOwnerMode, SessionRecordingFormat,
    SessionRecordingListResponse, SessionRecordingMode, SessionRecordingPolicy,
    SessionRecordingResource, SessionRecordingState, SessionRecordingTerminationReason,
    SessionResource, SetAutomationDelegateRequest, StoredSession, StoredSessionRecording,
};
use crate::session_files::{
    SessionFileBindingListResponse, SessionFileBindingResource, SessionFileListResponse,
    SessionFileResource,
};
use crate::session_hub::SessionTelemetrySnapshot;
use crate::session_manager::{SessionManagerError, SessionRuntime};
use crate::workflow::{
    derive_workflow_run_admission_resource, derive_workflow_run_intervention_resource,
    derive_workflow_run_runtime_resource, PersistWorkflowDefinitionRequest,
    PersistWorkflowDefinitionVersionRequest, PersistWorkflowRunEventRequest,
    PersistWorkflowRunLogRequest, PersistWorkflowRunProducedFileRequest, PersistWorkflowRunRequest,
    StoredWorkflowDefinition, StoredWorkflowDefinitionVersion, StoredWorkflowRun,
    WorkflowDefinitionListResponse, WorkflowDefinitionResource,
    WorkflowDefinitionVersionListResponse, WorkflowDefinitionVersionResource,
    WorkflowRunEventListResponse, WorkflowRunEventResource, WorkflowRunInterventionResource,
    WorkflowRunListResponse, WorkflowRunLogListResponse, WorkflowRunLogResource,
    WorkflowRunProducedFileResource, WorkflowRunRecordingResource, WorkflowRunResource,
    WorkflowRunRetentionResource, WorkflowRunSourceSnapshot, WorkflowRunState,
    WorkflowRunTransitionRequest, WorkflowRunWorkspaceInput,
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

mod admin_events;
mod authz;
mod automation_tasks;
mod credential_bindings;
mod errors;
mod extensions;
mod file_workspaces;
mod http_helpers;
mod recordings;
mod resources;
mod router;
mod runtime_access;
mod session_bindings;
mod session_files;
mod sessions;
mod types;
mod workflow_definitions;
mod workflow_events;
mod workflow_files;
mod workflow_run_operations;
mod workflows;

use authz::*;
use errors::*;
use http_helpers::*;
use resources::*;
use router::build_api_router;
use runtime_access::*;
use session_bindings::*;
pub(crate) use types::ApiServerConfig;
use types::*;

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

#[cfg(test)]
mod tests;
