use super::*;

impl SessionStore {
    pub async fn record_session_file(
        &self,
        request: PersistSessionFileRequest,
    ) -> Result<StoredSessionFile, SessionStoreError> {
        validate_session_file_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.record_session_file(request).await,
            SessionStoreBackend::Postgres(store) => store.record_session_file(request).await,
        }
    }

    pub async fn list_session_files_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionFile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_session_files_for_session(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_session_files_for_session(session_id).await
            }
        }
    }

    pub async fn get_session_file_for_session(
        &self,
        session_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredSessionFile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_session_file_for_session(session_id, file_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_session_file_for_session(session_id, file_id)
                    .await
            }
        }
    }

    pub async fn list_session_file_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<SessionFileRetentionCandidate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_session_file_retention_candidates(now, retention)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_session_file_retention_candidates(now, retention)
                    .await
            }
        }
    }

    pub async fn delete_session_file_for_session(
        &self,
        session_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredSessionFile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .delete_session_file_for_session(session_id, file_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .delete_session_file_for_session(session_id, file_id)
                    .await
            }
        }
    }

    pub async fn create_session_file_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        mut request: PersistSessionFileBindingRequest,
    ) -> Result<StoredSessionFileBinding, SessionStoreError> {
        validate_session_file_binding_request(&mut request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_session_file_binding_for_owner(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_session_file_binding_for_owner(principal, request)
                    .await
            }
        }
    }

    pub async fn list_session_file_bindings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionFileBinding>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_session_file_bindings_for_session(session_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_session_file_bindings_for_session(session_id)
                    .await
            }
        }
    }

    pub async fn get_session_file_binding_for_session(
        &self,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_session_file_binding_for_session(session_id, binding_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_session_file_binding_for_session(session_id, binding_id)
                    .await
            }
        }
    }

    pub async fn remove_session_file_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .remove_session_file_binding_for_owner(principal, session_id, binding_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .remove_session_file_binding_for_owner(principal, session_id, binding_id)
                    .await
            }
        }
    }

    pub async fn mark_session_file_binding_materialized(
        &self,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_session_file_binding_materialized(session_id, binding_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_session_file_binding_materialized(session_id, binding_id)
                    .await
            }
        }
    }

    pub async fn fail_session_file_binding_materialization(
        &self,
        session_id: Uuid,
        binding_id: Uuid,
        error: String,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        if error.trim().is_empty() {
            return Err(SessionStoreError::InvalidRequest(
                "session file binding materialization error must not be empty".to_string(),
            ));
        }
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .fail_session_file_binding_materialization(session_id, binding_id, error)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .fail_session_file_binding_materialization(session_id, binding_id, error)
                    .await
            }
        }
    }
}
