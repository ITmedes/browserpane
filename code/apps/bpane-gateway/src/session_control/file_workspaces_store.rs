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
