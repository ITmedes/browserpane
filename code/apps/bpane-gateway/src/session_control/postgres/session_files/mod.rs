use super::*;

pub(super) struct SessionFileRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn session_file_repository(&self) -> SessionFileRepository<'_> {
        SessionFileRepository { store: self }
    }

    pub(in crate::session_control) async fn create_session_file_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistSessionFileBindingRequest,
    ) -> Result<StoredSessionFileBinding, SessionStoreError> {
        self.session_file_repository()
            .create_session_file_binding_for_owner(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_session_file_bindings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionFileBinding>, SessionStoreError> {
        self.session_file_repository()
            .list_session_file_bindings_for_session(session_id)
            .await
    }

    pub(in crate::session_control) async fn get_session_file_binding_for_session(
        &self,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        self.session_file_repository()
            .get_session_file_binding_for_session(session_id, binding_id)
            .await
    }

    pub(in crate::session_control) async fn remove_session_file_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        self.session_file_repository()
            .remove_session_file_binding_for_owner(principal, session_id, binding_id)
            .await
    }
}

impl SessionFileRepository<'_> {
    pub(in crate::session_control) async fn create_session_file_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistSessionFileBindingRequest,
    ) -> Result<StoredSessionFileBinding, SessionStoreError> {
        let Some(_) = self
            .store
            .get_session_for_owner(principal, request.session_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "session {} not found",
                request.session_id
            )));
        };
        let Some(file) = self
            .store
            .get_file_workspace_file_for_owner(principal, request.workspace_id, request.file_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "file workspace file {} for workspace {} not found",
                request.file_id, request.workspace_id
            )));
        };

        let now = Utc::now();
        let row = self
            .store
            .db
            .client()
            .await?
            .query_one(
                r#"
                INSERT INTO control_session_file_bindings (
                    id,
                    session_id,
                    workspace_id,
                    file_id,
                    file_name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    mount_path,
                    mode,
                    state,
                    error,
                    labels,
                    created_at,
                    updated_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                    $11, $12, 'pending', NULL, $13, $14, $14
                )
                RETURNING
                    id,
                    session_id,
                    workspace_id,
                    file_id,
                    file_name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    mount_path,
                    mode,
                    state,
                    error,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[
                    &request.id,
                    &request.session_id,
                    &file.workspace_id,
                    &file.id,
                    &file.name,
                    &file.media_type,
                    &(file.byte_count as i64),
                    &file.sha256_hex,
                    &file.provenance,
                    &file.artifact_ref,
                    &request.mount_path,
                    &request.mode.as_str(),
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if let Some(db_error) = error.as_db_error() {
                    if db_error.constraint()
                        == Some("idx_control_session_file_bindings_active_mount_path")
                    {
                        return SessionStoreError::Conflict(format!(
                            "session file binding mount_path {} already exists for session {}",
                            request.mount_path, request.session_id
                        ));
                    }
                }
                SessionStoreError::Backend(format!(
                    "failed to create session file binding: {}",
                    describe_postgres_error(&error)
                ))
            })?;
        row_to_stored_session_file_binding(&row)
    }

    pub(in crate::session_control) async fn list_session_file_bindings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionFileBinding>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    session_id,
                    workspace_id,
                    file_id,
                    file_name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    mount_path,
                    mode,
                    state,
                    error,
                    labels,
                    created_at,
                    updated_at
                FROM control_session_file_bindings
                WHERE session_id = $1
                  AND state <> 'removed'
                ORDER BY created_at DESC
                "#,
                &[&session_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list session file bindings: {error}"))
            })?;
        rows.iter()
            .map(row_to_stored_session_file_binding)
            .collect()
    }

    pub(in crate::session_control) async fn get_session_file_binding_for_session(
        &self,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    id,
                    session_id,
                    workspace_id,
                    file_id,
                    file_name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    mount_path,
                    mode,
                    state,
                    error,
                    labels,
                    created_at,
                    updated_at
                FROM control_session_file_bindings
                WHERE session_id = $1
                  AND id = $2
                  AND state <> 'removed'
                "#,
                &[&session_id, &binding_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch session file binding: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_session_file_binding)
            .transpose()
    }

    pub(in crate::session_control) async fn remove_session_file_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        session_id: Uuid,
        binding_id: Uuid,
    ) -> Result<Option<StoredSessionFileBinding>, SessionStoreError> {
        if self
            .store
            .get_session_for_owner(principal, session_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }

        let now = Utc::now();
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_session_file_bindings
                SET state = 'removed',
                    updated_at = $3
                WHERE session_id = $1
                  AND id = $2
                RETURNING
                    id,
                    session_id,
                    workspace_id,
                    file_id,
                    file_name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    mount_path,
                    mode,
                    state,
                    error,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[&session_id, &binding_id, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to remove session file binding: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_session_file_binding)
            .transpose()
    }
}
