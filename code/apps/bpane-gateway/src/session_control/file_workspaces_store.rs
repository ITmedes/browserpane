use super::*;

impl InMemorySessionStore {
    pub(super) async fn create_file_workspace(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceRequest,
    ) -> Result<StoredFileWorkspace, SessionStoreError> {
        let now = Utc::now();
        let workspace = StoredFileWorkspace {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            labels: request.labels,
            created_at: now,
            updated_at: now,
        };
        self.file_workspaces.lock().await.push(workspace.clone());
        Ok(workspace)
    }

    pub(super) async fn list_file_workspaces_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredFileWorkspace>, SessionStoreError> {
        let mut workspaces = self
            .file_workspaces
            .lock()
            .await
            .iter()
            .filter(|workspace| {
                workspace.owner_subject == principal.subject
                    && workspace.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        workspaces.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(workspaces)
    }

    pub(super) async fn get_file_workspace_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredFileWorkspace>, SessionStoreError> {
        Ok(self
            .file_workspaces
            .lock()
            .await
            .iter()
            .find(|workspace| {
                workspace.id == id
                    && workspace.owner_subject == principal.subject
                    && workspace.owner_issuer == principal.issuer
            })
            .cloned())
    }

    pub(super) async fn create_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceFileRequest,
    ) -> Result<StoredFileWorkspaceFile, SessionStoreError> {
        let Some(workspace) = self
            .get_file_workspace_for_owner(principal, request.workspace_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "file workspace {} not found",
                request.workspace_id
            )));
        };

        let now = Utc::now();
        let file = StoredFileWorkspaceFile {
            id: request.id,
            workspace_id: workspace.id,
            name: request.name,
            media_type: request.media_type,
            byte_count: request.byte_count,
            sha256_hex: request.sha256_hex,
            provenance: request.provenance,
            artifact_ref: request.artifact_ref,
            created_at: now,
            updated_at: now,
        };
        self.file_workspace_files.lock().await.push(file.clone());
        Ok(file)
    }

    pub(super) async fn list_file_workspace_files_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
    ) -> Result<Vec<StoredFileWorkspaceFile>, SessionStoreError> {
        if self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut files = self
            .file_workspace_files
            .lock()
            .await
            .iter()
            .filter(|file| file.workspace_id == workspace_id)
            .cloned()
            .collect::<Vec<_>>();
        files.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(files)
    }

    pub(super) async fn get_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        if self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }

        Ok(self
            .file_workspace_files
            .lock()
            .await
            .iter()
            .find(|file| file.workspace_id == workspace_id && file.id == file_id)
            .cloned())
    }

    pub(super) async fn delete_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        if self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }

        let mut files = self.file_workspace_files.lock().await;
        let Some(index) = files
            .iter()
            .position(|file| file.workspace_id == workspace_id && file.id == file_id)
        else {
            return Ok(None);
        };
        Ok(Some(files.remove(index)))
    }
}

impl PostgresSessionStore {
    pub(super) async fn create_file_workspace(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceRequest,
    ) -> Result<StoredFileWorkspace, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .db
            .client()
            .await?
            .query_one(
                r#"
                INSERT INTO control_file_workspaces (
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    labels,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $7)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.description,
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to create file workspace: {error}"))
            })?;
        row_to_stored_file_workspace(&row)
    }

    pub(super) async fn list_file_workspaces_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredFileWorkspace>, SessionStoreError> {
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    labels,
                    created_at,
                    updated_at
                FROM control_file_workspaces
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list file workspaces: {error}"))
            })?;
        rows.iter().map(row_to_stored_file_workspace).collect()
    }

    pub(super) async fn get_file_workspace_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredFileWorkspace>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    labels,
                    created_at,
                    updated_at
                FROM control_file_workspaces
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch file workspace: {error}"))
            })?;
        row.as_ref().map(row_to_stored_file_workspace).transpose()
    }

    pub(super) async fn create_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistFileWorkspaceFileRequest,
    ) -> Result<StoredFileWorkspaceFile, SessionStoreError> {
        let Some(_) = self
            .get_file_workspace_for_owner(principal, request.workspace_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "file workspace {} not found",
                request.workspace_id
            )));
        };

        let now = Utc::now();
        let row = self
            .db
            .client()
            .await?
            .query_one(
                r#"
                INSERT INTO control_file_workspace_files (
                    id,
                    workspace_id,
                    name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $9)
                RETURNING
                    id,
                    workspace_id,
                    name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    created_at,
                    updated_at
                "#,
                &[
                    &request.id,
                    &request.workspace_id,
                    &request.name,
                    &request.media_type,
                    &(request.byte_count as i64),
                    &request.sha256_hex,
                    &request.provenance,
                    &request.artifact_ref,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to create workspace file: {error}"))
            })?;
        row_to_stored_file_workspace_file(&row)
    }

    pub(super) async fn list_file_workspace_files_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
    ) -> Result<Vec<StoredFileWorkspaceFile>, SessionStoreError> {
        if self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    file.id,
                    file.workspace_id,
                    file.name,
                    file.media_type,
                    file.byte_count,
                    file.sha256_hex,
                    file.provenance,
                    file.artifact_ref,
                    file.created_at,
                    file.updated_at
                FROM control_file_workspace_files file
                WHERE file.workspace_id = $1
                ORDER BY file.created_at DESC
                "#,
                &[&workspace_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workspace files: {error}"))
            })?;
        rows.iter().map(row_to_stored_file_workspace_file).collect()
    }

    pub(super) async fn get_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        let Some(_) = self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
        else {
            return Ok(None);
        };

        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    id,
                    workspace_id,
                    name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    created_at,
                    updated_at
                FROM control_file_workspace_files
                WHERE workspace_id = $1
                  AND id = $2
                "#,
                &[&workspace_id, &file_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch workspace file: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_file_workspace_file)
            .transpose()
    }

    pub(super) async fn delete_file_workspace_file_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workspace_id: Uuid,
        file_id: Uuid,
    ) -> Result<Option<StoredFileWorkspaceFile>, SessionStoreError> {
        let Some(_) = self
            .get_file_workspace_for_owner(principal, workspace_id)
            .await?
        else {
            return Ok(None);
        };

        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                DELETE FROM control_file_workspace_files
                WHERE workspace_id = $1
                  AND id = $2
                RETURNING
                    id,
                    workspace_id,
                    name,
                    media_type,
                    byte_count,
                    sha256_hex,
                    provenance,
                    artifact_ref,
                    created_at,
                    updated_at
                "#,
                &[&workspace_id, &file_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to delete workspace file: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_file_workspace_file)
            .transpose()
    }
}
