use super::*;

impl SessionStore {
    pub async fn create_identity_mapping(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistIdentityMappingRequest,
    ) -> Result<StoredIdentityMapping, SessionStoreError> {
        validate_identity_mapping_request(&request)?;
        self.validate_identity_mapping_references(principal, &request)
            .await?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_identity_mapping(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_identity_mapping(principal, request).await
            }
        }
    }

    pub async fn list_identity_mappings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredIdentityMapping>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_identity_mappings_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_identity_mappings_for_owner(principal).await
            }
        }
    }

    pub async fn get_identity_mapping_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredIdentityMapping>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_identity_mapping_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_identity_mapping_for_owner(principal, id).await
            }
        }
    }

    pub async fn update_identity_mapping_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistIdentityMappingRequest,
    ) -> Result<Option<StoredIdentityMapping>, SessionStoreError> {
        validate_identity_mapping_request(&request)?;
        self.validate_identity_mapping_references(principal, &request)
            .await?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .update_identity_mapping_for_owner(principal, id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .update_identity_mapping_for_owner(principal, id, request)
                    .await
            }
        }
    }

    async fn validate_identity_mapping_references(
        &self,
        principal: &AuthenticatedPrincipal,
        request: &PersistIdentityMappingRequest,
    ) -> Result<(), SessionStoreError> {
        let project = self
            .get_project_for_owner(principal, request.project_id)
            .await?
            .ok_or_else(|| {
                SessionStoreError::NotFound(format!(
                    "project {} not found for identity mapping",
                    request.project_id
                ))
            })?;
        if project.state == ProjectState::Archived {
            return Err(SessionStoreError::InvalidRequest(format!(
                "project {} is archived and cannot be used for identity mappings",
                request.project_id
            )));
        }

        if let Some(service_principal_id) = request.service_principal_id {
            let service_principal = self
                .get_service_principal_for_owner(principal, service_principal_id)
                .await?
                .ok_or_else(|| {
                    SessionStoreError::NotFound(format!(
                        "service principal {service_principal_id} not found for identity mapping"
                    ))
                })?;
            if request.kind != IdentityMappingKind::ServicePrincipal {
                return Ok(());
            }
            if service_principal.issuer != request.issuer
                || service_principal.client_id != request.external_id
            {
                return Err(SessionStoreError::InvalidRequest(format!(
                    "identity mapping external identity must match service principal {}",
                    service_principal_id
                )));
            }
        }
        Ok(())
    }

    pub async fn create_service_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistServicePrincipalRequest,
    ) -> Result<StoredServicePrincipal, SessionStoreError> {
        validate_service_principal_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_service_principal(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_service_principal(principal, request).await
            }
        }
    }

    pub async fn list_service_principals_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredServicePrincipal>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_service_principals_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_service_principals_for_owner(principal).await
            }
        }
    }

    pub async fn get_service_principal_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_service_principal_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_service_principal_for_owner(principal, id).await
            }
        }
    }

    pub async fn update_service_principal_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistServicePrincipalRequest,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        validate_service_principal_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .update_service_principal_for_owner(principal, id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .update_service_principal_for_owner(principal, id, request)
                    .await
            }
        }
    }

    pub async fn get_service_principal_for_owner_by_external_identity(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_service_principal_for_owner_by_external_identity(
                        principal, issuer, client_id,
                    )
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_service_principal_for_owner_by_external_identity(
                        principal, issuer, client_id,
                    )
                    .await
            }
        }
    }

    pub async fn mark_service_principal_seen_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_service_principal_seen_for_owner(principal, issuer, client_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_service_principal_seen_for_owner(principal, issuer, client_id)
                    .await
            }
        }
    }

    pub async fn mark_service_principal_delegated_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_service_principal_delegated_for_owner(principal, issuer, client_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_service_principal_delegated_for_owner(principal, issuer, client_id)
                    .await
            }
        }
    }

    pub async fn create_project(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistProjectRequest,
    ) -> Result<StoredProject, SessionStoreError> {
        validate_project_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.create_project(principal, request).await,
            SessionStoreBackend::Postgres(store) => store.create_project(principal, request).await,
        }
    }

    pub async fn list_projects_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredProject>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_projects_for_owner(principal).await,
            SessionStoreBackend::Postgres(store) => store.list_projects_for_owner(principal).await,
        }
    }

    pub async fn get_project_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredProject>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_project_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_project_for_owner(principal, id).await
            }
        }
    }

    pub async fn update_project_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistProjectRequest,
    ) -> Result<Option<StoredProject>, SessionStoreError> {
        validate_project_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.update_project_for_owner(principal, id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.update_project_for_owner(principal, id, request).await
            }
        }
    }

    pub async fn count_active_sessions_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .count_active_sessions_for_project(principal, project_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .count_active_sessions_for_project(principal, project_id)
                    .await
            }
        }
    }

    pub async fn count_active_workflow_runs_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .count_active_workflow_runs_for_project(principal, project_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .count_active_workflow_runs_for_project(principal, project_id)
                    .await
            }
        }
    }

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

    pub async fn create_egress_profile(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistEgressProfileRequest,
    ) -> Result<StoredEgressProfile, SessionStoreError> {
        validate_egress_profile_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_egress_profile(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_egress_profile(principal, request).await
            }
        }
    }

    pub async fn list_egress_profiles_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredEgressProfile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_egress_profiles_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_egress_profiles_for_owner(principal).await
            }
        }
    }

    pub async fn get_egress_profile_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredEgressProfile>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_egress_profile_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_egress_profile_for_owner(principal, id).await
            }
        }
    }

    pub async fn update_egress_profile_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistEgressProfileRequest,
    ) -> Result<Option<StoredEgressProfile>, SessionStoreError> {
        validate_egress_profile_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .update_egress_profile_for_owner(principal, id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .update_egress_profile_for_owner(principal, id, request)
                    .await
            }
        }
    }

    pub async fn upsert_egress_diagnostics_probe_result(
        &self,
        result: PersistEgressDiagnosticsProbeResult,
    ) -> Result<StoredEgressDiagnosticsProbeResult, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.upsert_egress_diagnostics_probe_result(result).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.upsert_egress_diagnostics_probe_result(result).await
            }
        }
    }

    pub async fn get_egress_diagnostics_probe_result_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredEgressDiagnosticsProbeResult>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_egress_diagnostics_probe_result_for_session(session_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_egress_diagnostics_probe_result_for_session(session_id)
                    .await
            }
        }
    }

    pub async fn upsert_egress_profile_reachability_probe_result(
        &self,
        result: PersistEgressProfileReachabilityProbeResult,
    ) -> Result<StoredEgressProfileReachabilityProbeResult, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .upsert_egress_profile_reachability_probe_result(result)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .upsert_egress_profile_reachability_probe_result(result)
                    .await
            }
        }
    }

    pub async fn get_egress_profile_reachability_probe_result(
        &self,
        profile_id: Uuid,
    ) -> Result<Option<StoredEgressProfileReachabilityProbeResult>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_egress_profile_reachability_probe_result(profile_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_egress_profile_reachability_probe_result(profile_id)
                    .await
            }
        }
    }

    pub async fn list_egress_profile_reachability_probe_results_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<HashMap<Uuid, StoredEgressProfileReachabilityProbeResult>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_egress_profile_reachability_probe_results_for_owner(principal)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_egress_profile_reachability_probe_results_for_owner(principal)
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
