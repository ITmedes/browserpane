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
use crate::credentials::provider::{CredentialProviderBackend, ResolvedCredentialSecret};
use crate::credentials::{
    CredentialProvider, CredentialProviderError, StoreCredentialSecretRequest,
    StoredCredentialSecret,
};
use crate::recording::{
    prepare_session_recording_playback, RecordingArtifactStore, RecordingObservability,
};
use crate::recording_lifecycle::RecordingLifecycleManager;
use crate::session_access::{SessionAutomationAccessTokenManager, SessionConnectTicketManager};
use crate::session_control::{
    SessionRecordingFormat, SessionRecordingMode, SessionRecordingPolicy,
    SessionRecordingState as StoredSessionRecordingState, StoredSessionRecording,
};
use crate::session_manager::{SessionManager, SessionManagerConfig, SessionManagerProfile};
use crate::workflow::{WorkflowObservability, WorkflowSourceResolver};
use crate::workflow_lifecycle::{WorkflowLifecycleManager, WorkflowWorkerConfig};
use crate::workspaces::WorkspaceFileStore;

mod support;
pub(crate) use support::*;

mod automation_tasks;
mod credential_bindings;
mod extensions;
mod file_workspaces;
mod recordings;
mod sessions;
mod workflow_definitions;
mod workflow_events;
mod workflow_files;
mod workflow_run_operations;
mod workflow_run_state;
mod workflow_runtime;
mod workflows;
