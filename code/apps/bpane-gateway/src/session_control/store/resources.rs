use super::*;

impl SessionStore {
    pub fn validate_browser_context_request(
        request: &PersistBrowserContextRequest,
    ) -> Result<(), SessionStoreError> {
        validate_browser_context_request(request)
    }

    pub async fn create_browser_context(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistBrowserContextRequest,
    ) -> Result<StoredBrowserContext, SessionStoreError> {
        validate_browser_context_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_browser_context(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_browser_context(principal, request).await
            }
        }
    }

    pub async fn list_browser_contexts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredBrowserContext>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_browser_contexts_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_browser_contexts_for_owner(principal).await
            }
        }
    }

    pub async fn get_browser_context_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_browser_context_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_browser_context_for_owner(principal, id).await
            }
        }
    }

    pub async fn mark_browser_context_used_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_browser_context_used_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_browser_context_used_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn delete_browser_context_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.delete_browser_context_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.delete_browser_context_for_owner(principal, id).await
            }
        }
    }

    pub async fn list_browser_context_retention_candidates(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<BrowserContextRetentionCandidate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_browser_context_retention_candidates(now).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_browser_context_retention_candidates(now).await
            }
        }
    }

    pub async fn create_session_template(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistSessionTemplateRequest,
    ) -> Result<StoredSessionTemplate, SessionStoreError> {
        validate_session_template_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_session_template(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_session_template(principal, request).await
            }
        }
    }

    pub async fn list_session_templates_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSessionTemplate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_session_templates_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_session_templates_for_owner(principal).await
            }
        }
    }

    pub async fn get_session_template_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSessionTemplate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_session_template_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_session_template_for_owner(principal, id).await
            }
        }
    }

    pub async fn update_session_template_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistSessionTemplateRequest,
    ) -> Result<Option<StoredSessionTemplate>, SessionStoreError> {
        validate_session_template_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .update_session_template_for_owner(principal, id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .update_session_template_for_owner(principal, id, request)
                    .await
            }
        }
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
}
