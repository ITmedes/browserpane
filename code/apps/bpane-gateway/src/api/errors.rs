use axum::http::StatusCode;
use axum::Json;
use serde::Serialize;

use crate::credentials::CredentialProviderError;
use crate::recording::{RecordingArtifactStoreError, RecordingPlaybackError};
use crate::recording_lifecycle::RecordingLifecycleError;
use crate::session_control::SessionStoreError;
use crate::workflow::WorkflowSourceError;
use crate::workspaces::WorkspaceFileStoreError;

#[derive(Serialize)]
pub(super) struct ErrorResponse {
    pub error: String,
}

pub(super) fn map_session_store_error(
    error: SessionStoreError,
) -> (StatusCode, Json<ErrorResponse>) {
    match error {
        SessionStoreError::ActiveSessionConflict { .. } | SessionStoreError::Conflict(_) => (
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

pub(super) fn map_recording_artifact_store_error(
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

pub(super) fn map_workspace_file_store_error(
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

pub(super) fn map_workspace_file_content_error(
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

pub(super) fn map_credential_provider_error(
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

pub(super) fn map_workflow_source_error(
    error: WorkflowSourceError,
) -> (StatusCode, Json<ErrorResponse>) {
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

pub(super) fn map_recording_playback_error(
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

pub(super) fn map_recording_lifecycle_error(
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
