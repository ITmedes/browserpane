use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn record_session_file(
        &self,
        request: PersistSessionFileRequest,
    ) -> Result<StoredSessionFile, SessionStoreError> {
        let Some(session) = self.get_session_by_id(request.session_id).await? else {
            return Err(SessionStoreError::NotFound(format!(
                "session {} not found",
                request.session_id
            )));
        };

        let now = Utc::now();
        let file = StoredSessionFile {
            id: request.id,
            session_id: session.id,
            owner_subject: session.owner.subject.clone(),
            owner_issuer: session.owner.issuer.clone(),
            name: request.name,
            media_type: request.media_type,
            byte_count: request.byte_count,
            sha256_hex: request.sha256_hex,
            artifact_ref: request.artifact_ref,
            source: request.source,
            labels: request.labels,
            created_at: now,
            updated_at: now,
        };
        self.session_files.lock().await.push(file.clone());
        Ok(file)
    }

    pub(in crate::session_control) async fn list_session_files_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionFile>, SessionStoreError> {
        let mut files = self
            .session_files
            .lock()
            .await
            .iter()
            .filter(|file| file.session_id == session_id)
            .cloned()
            .collect::<Vec<_>>();
        files.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(files)
    }

    pub(in crate::session_control) async fn get_session_file_for_session(
        &self,
        session_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredSessionFile>, SessionStoreError> {
        Ok(self
            .session_files
            .lock()
            .await
            .iter()
            .find(|file| file.session_id == session_id && file.id == file_id)
            .cloned())
    }

    pub(in crate::session_control) async fn list_session_file_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<SessionFileRetentionCandidate>, SessionStoreError> {
        let mut candidates = self
            .session_files
            .lock()
            .await
            .iter()
            .filter_map(|file| {
                let expires_at = file.created_at + retention;
                if expires_at > now {
                    return None;
                }
                Some(SessionFileRetentionCandidate {
                    session_id: file.session_id,
                    file_id: file.id,
                    artifact_ref: file.artifact_ref.clone(),
                    expires_at,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| left.expires_at.cmp(&right.expires_at));
        Ok(candidates)
    }

    pub(in crate::session_control) async fn delete_session_file_for_session(
        &self,
        session_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredSessionFile>, SessionStoreError> {
        let mut files = self.session_files.lock().await;
        let Some(index) = files
            .iter()
            .position(|file| file.session_id == session_id && file.id == file_id)
        else {
            return Ok(None);
        };
        Ok(Some(files.remove(index)))
    }

    pub(in crate::session_control) async fn create_session_file_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistSessionFileBindingRequest,
    ) -> Result<StoredSessionFileBinding, SessionStoreError> {
        let Some(session) = self
            .get_session_for_owner(principal, request.session_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "session {} not found",
                request.session_id
            )));
        };
        let Some(file) = self
            .get_file_workspace_file_for_owner(principal, request.workspace_id, request.file_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "file workspace file {} for workspace {} not found",
                request.file_id, request.workspace_id
            )));
        };

        let mut bindings = self.session_file_bindings.lock().await;
        if bindings.iter().any(|binding| {
            binding.session_id == session.id
                && binding.mount_path == request.mount_path
                && binding.state != SessionFileBindingState::Removed
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "session file binding mount_path {} already exists for session {}",
                request.mount_path, session.id
            )));
        }

        let now = Utc::now();
        let binding = StoredSessionFileBinding {
            id: request.id,
            session_id: session.id,
            workspace_id: file.workspace_id,
            file_id: file.id,
            file_name: file.name,
            media_type: file.media_type,
            byte_count: file.byte_count,
            sha256_hex: file.sha256_hex,
            provenance: file.provenance,
            artifact_ref: file.artifact_ref,
            mount_path: request.mount_path,
            mode: request.mode,
            state: SessionFileBindingState::Pending,
            error: None,
            labels: request.labels,
            created_at: now,
            updated_at: now,
        };
        bindings.push(binding.clone());
        Ok(binding)
    }

    pub(in crate::session_control) async fn list_session_file_bindings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionFileBinding>, SessionStoreError> {
        let mut bindings = self
            .session_file_bindings
            .lock()
            .await
            .iter()
            .filter(|binding| {
                binding.session_id == session_id
                    && binding.state != SessionFileBindingState::Removed
            })
            .cloned()
            .collect::<Vec<_>>();
        bindings.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(bindings)
    }

    pub(in crate::session_control) async fn get_session_file_binding_for_session(
        &self,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        Ok(self
            .session_file_bindings
            .lock()
            .await
            .iter()
            .find(|binding| {
                binding.session_id == session_id
                    && binding.id == binding_id
                    && binding.state != SessionFileBindingState::Removed
            })
            .cloned())
    }

    pub(in crate::session_control) async fn remove_session_file_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        if self
            .get_session_for_owner(principal, session_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }

        let mut bindings = self.session_file_bindings.lock().await;
        let Some(binding) = bindings
            .iter_mut()
            .find(|binding| binding.session_id == session_id && binding.id == binding_id)
        else {
            return Ok(None);
        };
        binding.state = SessionFileBindingState::Removed;
        binding.updated_at = Utc::now();
        Ok(Some(binding.clone()))
    }

    pub(in crate::session_control) async fn mark_session_file_binding_materialized(
        &self,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        self.transition_session_file_binding_materialization(
            session_id,
            binding_id,
            SessionFileBindingState::Materialized,
            None,
        )
        .await
    }

    pub(in crate::session_control) async fn fail_session_file_binding_materialization(
        &self,
        session_id: Uuid,
        binding_id: Uuid,
        error: String,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        self.transition_session_file_binding_materialization(
            session_id,
            binding_id,
            SessionFileBindingState::Failed,
            Some(error),
        )
        .await
    }

    async fn transition_session_file_binding_materialization(
        &self,
        session_id: Uuid,
        binding_id: Uuid,
        state: SessionFileBindingState,
        error: Option<String>,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        let mut bindings = self.session_file_bindings.lock().await;
        let Some(binding) = bindings
            .iter_mut()
            .find(|binding| binding.session_id == session_id && binding.id == binding_id)
        else {
            return Ok(None);
        };
        if binding.state == SessionFileBindingState::Removed {
            return Ok(None);
        }
        binding.state = state;
        binding.error = error;
        binding.updated_at = Utc::now();
        Ok(Some(binding.clone()))
    }
}
