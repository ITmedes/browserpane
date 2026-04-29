use super::*;

mod workflows;

mod automation_tasks;

mod sessions;

#[derive(Debug, Clone)]
pub enum SessionStoreError {
    ActiveSessionConflict { max_runtime_sessions: usize },
    Conflict(String),
    NotFound(String),
    InvalidRequest(String),
    Backend(String),
}

impl std::fmt::Display for SessionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActiveSessionConflict {
                max_runtime_sessions,
            } => {
                write!(
                    f,
                    "the current gateway runtime only supports {} active runtime-backed session{}",
                    max_runtime_sessions,
                    if *max_runtime_sessions == 1 { "" } else { "s" }
                )
            }
            Self::Conflict(message) => write!(f, "{message}"),
            Self::NotFound(message) => write!(f, "{message}"),
            Self::InvalidRequest(message) => write!(f, "{message}"),
            Self::Backend(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SessionStoreError {}

#[derive(Clone)]
pub struct SessionStore {
    backend: SessionStoreBackend,
}

#[derive(Debug, Clone)]
pub(super) struct SessionStoreConfig {
    pub(super) runtime_binding: String,
    pub(super) max_runtime_candidates: usize,
}

#[derive(Clone)]
enum SessionStoreBackend {
    InMemory(Arc<InMemorySessionStore>),
    Postgres(Arc<PostgresSessionStore>),
}

impl From<SessionManagerProfile> for SessionStoreConfig {
    fn from(runtime_profile: SessionManagerProfile) -> Self {
        Self {
            runtime_binding: runtime_profile.runtime_binding,
            max_runtime_candidates: runtime_profile.max_runtime_sessions,
        }
    }
}

#[cfg(test)]
fn legacy_runtime_profile() -> SessionManagerProfile {
    SessionManagerProfile {
        runtime_binding: "legacy_single_session".to_string(),
        compatibility_mode: "legacy_single_runtime".to_string(),
        max_runtime_sessions: 1,
        supports_legacy_global_routes: true,
        supports_session_extensions: false,
    }
}

impl SessionStore {
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::in_memory_with_config(legacy_runtime_profile())
    }

    pub fn in_memory_with_config(runtime_profile: SessionManagerProfile) -> Self {
        Self {
            backend: SessionStoreBackend::InMemory(Arc::new(InMemorySessionStore::new(
                SessionStoreConfig::from(runtime_profile),
            ))),
        }
    }

    pub async fn from_database_url_with_config(
        database_url: &str,
        runtime_profile: SessionManagerProfile,
    ) -> Result<Self, SessionStoreError> {
        run_postgres_migrations(database_url).await?;
        let store =
            PostgresSessionStore::connect(database_url, SessionStoreConfig::from(runtime_profile))
                .await?;
        Ok(Self {
            backend: SessionStoreBackend::Postgres(Arc::new(store)),
        })
    }

    pub async fn create_file_workspace(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceRequest,
    ) -> Result<StoredFileWorkspace, SessionStoreError> {
        validate_file_workspace_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_file_workspace(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_file_workspace(principal, request).await
            }
        }
    }

    pub async fn create_credential_binding(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistCredentialBindingRequest,
    ) -> Result<StoredCredentialBinding, SessionStoreError> {
        validate_credential_binding_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_credential_binding(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_credential_binding(principal, request).await
            }
        }
    }

    pub async fn list_credential_bindings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredCredentialBinding>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_credential_bindings_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_credential_bindings_for_owner(principal).await
            }
        }
    }

    pub async fn get_credential_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredCredentialBinding>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_credential_binding_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_credential_binding_for_owner(principal, id).await
            }
        }
    }

    pub async fn create_extension_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionDefinitionRequest,
    ) -> Result<StoredExtensionDefinition, SessionStoreError> {
        validate_extension_definition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_extension_definition(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_extension_definition(principal, request).await
            }
        }
    }

    pub async fn list_extension_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredExtensionDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_extension_definitions_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_extension_definitions_for_owner(principal).await
            }
        }
    }

    pub async fn get_extension_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_extension_definition_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_extension_definition_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn set_extension_definition_enabled_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        enabled: bool,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .set_extension_definition_enabled_for_owner(principal, id, enabled)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .set_extension_definition_enabled_for_owner(principal, id, enabled)
                    .await
            }
        }
    }

    pub async fn create_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionVersionRequest,
    ) -> Result<StoredExtensionVersion, SessionStoreError> {
        validate_extension_version_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_extension_version_for_owner(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_extension_version_for_owner(principal, request)
                    .await
            }
        }
    }

    pub async fn get_latest_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        extension_definition_id: Uuid,
    ) -> Result<Option<StoredExtensionVersion>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_latest_extension_version_for_owner(principal, extension_definition_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_latest_extension_version_for_owner(principal, extension_definition_id)
                    .await
            }
        }
    }

    pub async fn list_file_workspaces_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredFileWorkspace>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_file_workspaces_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_file_workspaces_for_owner(principal).await
            }
        }
    }

    pub async fn get_file_workspace_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredFileWorkspace>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_file_workspace_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_file_workspace_for_owner(principal, id).await
            }
        }
    }

    pub async fn create_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceFileRequest,
    ) -> Result<StoredFileWorkspaceFile, SessionStoreError> {
        validate_file_workspace_file_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_file_workspace_file_for_owner(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_file_workspace_file_for_owner(principal, request)
                    .await
            }
        }
    }

    pub async fn list_file_workspace_files_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
    ) -> Result<Vec<StoredFileWorkspaceFile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_file_workspace_files_for_owner(principal, workspace_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_file_workspace_files_for_owner(principal, workspace_id)
                    .await
            }
        }
    }

    pub async fn get_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_file_workspace_file_for_owner(principal, workspace_id, file_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_file_workspace_file_for_owner(principal, workspace_id, file_id)
                    .await
            }
        }
    }

    pub async fn delete_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .delete_file_workspace_file_for_owner(principal, workspace_id, file_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .delete_file_workspace_file_for_owner(principal, workspace_id, file_id)
                    .await
            }
        }
    }

    pub async fn create_recording_for_session(
        &self,
        session_id: Uuid,
        format: SessionRecordingFormat,
        previous_recording_id: Option<Uuid>,
    ) -> Result<StoredSessionRecording, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_recording_for_session(session_id, format, previous_recording_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_recording_for_session(session_id, format, previous_recording_id)
                    .await
            }
        }
    }

    pub async fn list_recordings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_recordings_for_session(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_recordings_for_session(session_id).await
            }
        }
    }

    pub async fn get_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_recording_for_session(session_id, recording_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_recording_for_session(session_id, recording_id)
                    .await
            }
        }
    }

    pub async fn get_latest_recording_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_latest_recording_for_session(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_latest_recording_for_session(session_id).await
            }
        }
    }

    pub async fn list_recording_artifact_retention_candidates(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecordingArtifactRetentionCandidate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_recording_artifact_retention_candidates(now)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_recording_artifact_retention_candidates(now)
                    .await
            }
        }
    }

    pub async fn stop_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        termination_reason: SessionRecordingTerminationReason,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .stop_recording_for_session(session_id, recording_id, termination_reason)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .stop_recording_for_session(session_id, recording_id, termination_reason)
                    .await
            }
        }
    }

    pub async fn clear_recording_artifact_path(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .clear_recording_artifact_path(session_id, recording_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .clear_recording_artifact_path(session_id, recording_id)
                    .await
            }
        }
    }

    pub async fn complete_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: PersistCompletedSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        validate_persist_completed_recording_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .complete_recording_for_session(session_id, recording_id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .complete_recording_for_session(session_id, recording_id, request)
                    .await
            }
        }
    }

    pub async fn fail_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: FailSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        validate_fail_recording_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .fail_recording_for_session(session_id, recording_id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .fail_recording_for_session(session_id, recording_id, request)
                    .await
            }
        }
    }

    pub async fn upsert_recording_worker_assignment(
        &self,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.upsert_recording_worker_assignment(assignment).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.upsert_recording_worker_assignment(assignment).await
            }
        }
    }

    pub async fn clear_recording_worker_assignment(
        &self,
        session_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.clear_recording_worker_assignment(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.clear_recording_worker_assignment(session_id).await
            }
        }
    }

    pub async fn get_recording_worker_assignment(
        &self,
        session_id: Uuid,
    ) -> Result<Option<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_recording_worker_assignment(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_recording_worker_assignment(session_id).await
            }
        }
    }

    pub async fn list_recording_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_recording_worker_assignments().await,
            SessionStoreBackend::Postgres(store) => store.list_recording_worker_assignments().await,
        }
    }
}
