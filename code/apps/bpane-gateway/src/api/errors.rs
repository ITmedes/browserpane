use axum::http::StatusCode;
use axum::Json;
use serde::ser::{SerializeStruct, Serializer};
use serde::Serialize;

use crate::credentials::CredentialProviderError;
use crate::recording::{RecordingArtifactStoreError, RecordingPlaybackError};
use crate::recording_lifecycle::RecordingLifecycleError;
use crate::session_control::SessionStoreError;
use crate::workflow::WorkflowSourceError;
use crate::workspaces::WorkspaceFileStoreError;

pub(super) struct ErrorResponse {
    pub error: String,
}

impl Serialize for ErrorResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let metadata = error_metadata(&self.error);
        let mut state =
            serializer.serialize_struct("ErrorResponse", if metadata.is_some() { 4 } else { 1 })?;
        state.serialize_field("error", &self.error)?;
        if let Some(metadata) = metadata {
            state.serialize_field("code", metadata.code)?;
            state.serialize_field("category", metadata.category)?;
            state.serialize_field("recovery_hint", metadata.recovery_hint)?;
        }
        state.end()
    }
}

struct ErrorMetadata {
    code: &'static str,
    category: &'static str,
    recovery_hint: &'static str,
}

fn error_metadata(error: &str) -> Option<ErrorMetadata> {
    if error.starts_with("invalid workflow source:") {
        return Some(ErrorMetadata {
            code: "workflow_source_invalid",
            category: "workflow_source",
            recovery_hint:
                "Check the workflow source repository URL, ref, root_path, and entrypoint.",
        });
    }
    if error.starts_with("failed to resolve workflow source:") {
        return Some(ErrorMetadata {
            code: "workflow_source_ref_resolution_failed",
            category: "workflow_source",
            recovery_hint:
                "Check that the referenced branch, tag, or commit exists and can be resolved.",
        });
    }
    if error.starts_with("failed to access workflow source repository:") {
        return Some(ErrorMetadata {
            code: "workflow_source_repository_access_failed",
            category: "workflow_source",
            recovery_hint: "Check repository access, credentials, network reachability, and local git safe.directory configuration.",
        });
    }
    if error.starts_with("failed to materialize workflow source:") {
        return Some(ErrorMetadata {
            code: "workflow_source_materialization_failed",
            category: "workflow_source",
            recovery_hint: "Check that the pinned commit, root_path, and entrypoint can be checked out and read.",
        });
    }
    if error.starts_with("failed to create workflow source snapshot:") {
        return Some(ErrorMetadata {
            code: "workflow_source_snapshot_failed",
            category: "workflow_source",
            recovery_hint:
                "Check source file permissions and the local workflow snapshot workspace.",
        });
    }
    if error.starts_with("workflow source infrastructure unavailable:") {
        return Some(ErrorMetadata {
            code: "workflow_source_infrastructure_unavailable",
            category: "workflow_source",
            recovery_hint: "Check that git is installed in the gateway image and that local runtime dependencies are available.",
        });
    }
    None
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
    match &error {
        WorkflowSourceError::Invalid(_) => (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        WorkflowSourceError::Resolve(_) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        WorkflowSourceError::RepositoryAccess(_) => (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: error.to_string(),
            }),
        ),
        WorkflowSourceError::Materialize(_)
        | WorkflowSourceError::Snapshot(_)
        | WorkflowSourceError::Infrastructure(_) => (
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

#[cfg(test)]
mod tests {
    use axum::Json;
    use serde_json::json;

    use super::*;

    #[test]
    fn serializes_workflow_source_error_metadata_without_dropping_error_string() {
        let response = ErrorResponse {
            error: WorkflowSourceError::RepositoryAccess(
                "git ls-remote failed for /workspace: detected dubious ownership".to_string(),
            )
            .to_string(),
        };

        assert_eq!(
            serde_json::to_value(response).unwrap(),
            json!({
                "error": "failed to access workflow source repository: git ls-remote failed for /workspace: detected dubious ownership",
                "code": "workflow_source_repository_access_failed",
                "category": "workflow_source",
                "recovery_hint": "Check repository access, credentials, network reachability, and local git safe.directory configuration.",
            })
        );
    }

    #[test]
    fn maps_workflow_source_error_status_by_failure_kind() {
        let (invalid_status, _) =
            map_workflow_source_error(WorkflowSourceError::Invalid("bad source".to_string()));
        let (resolve_status, _) =
            map_workflow_source_error(WorkflowSourceError::Resolve("missing ref".to_string()));
        let (repository_status, Json(repository_error)) = map_workflow_source_error(
            WorkflowSourceError::RepositoryAccess("private repository".to_string()),
        );
        let (infrastructure_status, _) = map_workflow_source_error(
            WorkflowSourceError::Infrastructure("git executable missing".to_string()),
        );

        assert_eq!(invalid_status, StatusCode::BAD_REQUEST);
        assert_eq!(resolve_status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(repository_status, StatusCode::BAD_GATEWAY);
        assert_eq!(
            repository_error.error,
            "failed to access workflow source repository: private repository"
        );
        assert_eq!(infrastructure_status, StatusCode::SERVICE_UNAVAILABLE);
    }
}
