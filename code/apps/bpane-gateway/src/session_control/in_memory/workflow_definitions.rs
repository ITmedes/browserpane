use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_workflow_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionRequest,
    ) -> Result<StoredWorkflowDefinition, SessionStoreError> {
        let now = Utc::now();
        let workflow = StoredWorkflowDefinition {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            labels: request.labels,
            latest_version: None,
            created_at: now,
            updated_at: now,
        };
        self.workflow_definitions
            .lock()
            .await
            .push(workflow.clone());
        Ok(workflow)
    }

    pub(in crate::session_control) async fn list_workflow_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowDefinition>, SessionStoreError> {
        let mut workflows = self
            .workflow_definitions
            .lock()
            .await
            .iter()
            .filter(|workflow| {
                workflow.owner_subject == principal.subject
                    && workflow.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        workflows.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(workflows)
    }

    pub(in crate::session_control) async fn get_workflow_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinition>, SessionStoreError> {
        Ok(self
            .workflow_definitions
            .lock()
            .await
            .iter()
            .find(|workflow| {
                workflow.id == id
                    && workflow.owner_subject == principal.subject
                    && workflow.owner_issuer == principal.issuer
            })
            .cloned())
    }

    pub(in crate::session_control) async fn create_workflow_definition_version(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionVersionRequest,
    ) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
        let Some(_) = self
            .get_workflow_definition_for_owner(principal, request.workflow_definition_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition {} not found",
                request.workflow_definition_id
            )));
        };

        let mut versions = self.workflow_definition_versions.lock().await;
        if versions.iter().any(|version| {
            version.workflow_definition_id == request.workflow_definition_id
                && version.version == request.version
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "workflow version {} already exists",
                request.version
            )));
        }

        let now = Utc::now();
        let version = StoredWorkflowDefinitionVersion {
            id: Uuid::now_v7(),
            workflow_definition_id: request.workflow_definition_id,
            version: request.version.clone(),
            executor: request.executor,
            entrypoint: request.entrypoint,
            source: request.source,
            input_schema: request.input_schema,
            output_schema: request.output_schema,
            default_session: request.default_session,
            allowed_credential_binding_ids: request.allowed_credential_binding_ids,
            allowed_extension_ids: request.allowed_extension_ids,
            allowed_file_workspace_ids: request.allowed_file_workspace_ids,
            created_at: now,
        };
        versions.push(version.clone());
        drop(versions);

        if let Some(workflow) = self
            .workflow_definitions
            .lock()
            .await
            .iter_mut()
            .find(|workflow| workflow.id == request.workflow_definition_id)
        {
            workflow.latest_version = Some(version.version.clone());
            workflow.updated_at = now;
        }

        Ok(version)
    }

    pub(in crate::session_control) async fn get_workflow_definition_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
        version: &str,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        if self
            .get_workflow_definition_for_owner(principal, workflow_definition_id)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        Ok(self
            .workflow_definition_versions
            .lock()
            .await
            .iter()
            .find(|stored| {
                stored.workflow_definition_id == workflow_definition_id && stored.version == version
            })
            .cloned())
    }

    pub(in crate::session_control) async fn list_workflow_definition_versions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
    ) -> Result<Vec<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        if self
            .get_workflow_definition_for_owner(principal, workflow_definition_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let mut versions = self
            .workflow_definition_versions
            .lock()
            .await
            .iter()
            .filter(|stored| stored.workflow_definition_id == workflow_definition_id)
            .cloned()
            .collect::<Vec<_>>();
        versions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(versions)
    }

    pub(in crate::session_control) async fn get_workflow_definition_version_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        Ok(self
            .workflow_definition_versions
            .lock()
            .await
            .iter()
            .find(|version| version.id == id)
            .cloned())
    }
}
