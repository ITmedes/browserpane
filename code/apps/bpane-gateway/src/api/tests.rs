use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::os::unix::fs::PermissionsExt;
use std::process::Command as StdCommand;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{Request, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use tokio::sync::{oneshot, Mutex};
use tokio::time::sleep;
use tower::ServiceExt;
use zip::ZipArchive;

use super::*;
use crate::auth::AuthValidator;
use crate::automation_access_token::SessionAutomationAccessTokenManager;
use crate::connect_ticket::SessionConnectTicketManager;
use crate::credential_provider::{
    CredentialProvider, CredentialProviderBackend, CredentialProviderError,
    ResolvedCredentialSecret, StoreCredentialSecretRequest, StoredCredentialSecret,
};
use crate::recording_artifact_store::RecordingArtifactStore;
use crate::recording_lifecycle::RecordingLifecycleManager;
use crate::recording_observability::RecordingObservability;
use crate::recording_playback::prepare_session_recording_playback;
use crate::recording_retention::RecordingRetentionManager;
use crate::session_control::{
    SessionRecordingFormat, SessionRecordingMode, SessionRecordingPolicy,
    SessionRecordingState as StoredSessionRecordingState, StoredSessionRecording,
};
use crate::session_manager::{SessionManager, SessionManagerConfig, SessionManagerProfile};
use crate::workflow_lifecycle::{WorkflowLifecycleManager, WorkflowWorkerConfig};
use crate::workflow_observability::WorkflowObservability;
use crate::workflow_source::WorkflowSourceResolver;
use crate::workspace_file_store::WorkspaceFileStore;

mod support;
pub(crate) use support::*;

mod automation_tasks;
mod credential_bindings;
mod extensions;
mod file_workspaces;
mod recordings;
mod sessions;
mod workflow_events;
mod workflow_run_operations;
mod workflow_run_state;
mod workflow_runtime;
mod workflows;
