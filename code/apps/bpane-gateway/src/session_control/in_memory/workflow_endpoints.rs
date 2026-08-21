use super::*;

fn owned_endpoint_matches(
    endpoint: &StoredWorkflowEndpoint,
    principal: &AuthenticatedPrincipal,
) -> bool {
    endpoint.owner_subject == principal.subject && endpoint.owner_issuer == principal.issuer
}

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_workflow_endpoint(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEndpointRequest,
    ) -> Result<StoredWorkflowEndpoint, SessionStoreError> {
        if self
            .get_project_for_owner(principal, request.project_id)
            .await?
            .is_none()
        {
            return Err(SessionStoreError::NotFound(format!(
                "project {} not found",
                request.project_id
            )));
        }
        let mut endpoints = self.workflow_endpoints.lock().await;
        if endpoints.iter().any(|endpoint| {
            endpoint.project_id == request.project_id
                && endpoint.endpoint_key == request.endpoint_key
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "workflow endpoint {} already exists in project {}",
                request.endpoint_key, request.project_id
            )));
        }
        let now = Utc::now();
        let endpoint = StoredWorkflowEndpoint {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            project_id: request.project_id,
            endpoint_key: request.endpoint_key,
            purpose: request.purpose,
            workflow_definition_id: request.workflow_definition_id,
            workflow_definition_version_id: request.workflow_definition_version_id,
            workflow_version: request.workflow_version,
            input_schema: request.input_schema,
            output_schema: request.output_schema,
            execution_timeout_seconds: request.execution_timeout_seconds,
            inline_result_max_bytes: request.inline_result_max_bytes,
            artifact_behavior: request.artifact_behavior,
            labels: request.labels,
            state: WorkflowEndpointState::Draft,
            created_at: now,
            updated_at: now,
        };
        endpoints.push(endpoint.clone());
        Ok(endpoint)
    }

    pub(in crate::session_control) async fn list_workflow_endpoints_for_owner_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEndpoint>, SessionStoreError> {
        let mut endpoints = self
            .workflow_endpoints
            .lock()
            .await
            .iter()
            .filter(|endpoint| {
                endpoint.project_id == project_id && owned_endpoint_matches(endpoint, principal)
            })
            .cloned()
            .collect::<Vec<_>>();
        endpoints.sort_by(|left, right| left.endpoint_key.cmp(&right.endpoint_key));
        Ok(endpoints)
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_for_owner_project_key(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
        endpoint_key: &str,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        Ok(self
            .workflow_endpoints
            .lock()
            .await
            .iter()
            .find(|endpoint| {
                endpoint.project_id == project_id
                    && endpoint.endpoint_key == endpoint_key
                    && owned_endpoint_matches(endpoint, principal)
            })
            .cloned())
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_by_project_key(
        &self,
        project_id: Uuid,
        endpoint_key: &str,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        Ok(self
            .workflow_endpoints
            .lock()
            .await
            .iter()
            .find(|endpoint| {
                endpoint.project_id == project_id && endpoint.endpoint_key == endpoint_key
            })
            .cloned())
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_by_id(
        &self,
        endpoint_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        Ok(self
            .workflow_endpoints
            .lock()
            .await
            .iter()
            .find(|endpoint| endpoint.id == endpoint_id)
            .cloned())
    }

    pub(in crate::session_control) async fn update_workflow_endpoint_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint_id: Uuid,
        request: PersistWorkflowEndpointRequest,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        let mut endpoints = self.workflow_endpoints.lock().await;
        let Some(endpoint) = endpoints.iter_mut().find(|endpoint| {
            endpoint.id == endpoint_id && owned_endpoint_matches(endpoint, principal)
        }) else {
            return Ok(None);
        };
        endpoint.purpose = request.purpose;
        endpoint.workflow_definition_id = request.workflow_definition_id;
        endpoint.workflow_definition_version_id = request.workflow_definition_version_id;
        endpoint.workflow_version = request.workflow_version;
        endpoint.input_schema = request.input_schema;
        endpoint.output_schema = request.output_schema;
        endpoint.execution_timeout_seconds = request.execution_timeout_seconds;
        endpoint.inline_result_max_bytes = request.inline_result_max_bytes;
        endpoint.artifact_behavior = request.artifact_behavior;
        endpoint.labels = request.labels;
        endpoint.updated_at = Utc::now();
        Ok(Some(endpoint.clone()))
    }

    pub(in crate::session_control) async fn set_workflow_endpoint_state_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint_id: Uuid,
        state: WorkflowEndpointState,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        let mut endpoints = self.workflow_endpoints.lock().await;
        let Some(endpoint) = endpoints.iter_mut().find(|endpoint| {
            endpoint.id == endpoint_id && owned_endpoint_matches(endpoint, principal)
        }) else {
            return Ok(None);
        };
        endpoint.state = state;
        endpoint.updated_at = Utc::now();
        Ok(Some(endpoint.clone()))
    }

    pub(in crate::session_control) async fn upsert_workflow_endpoint_grant_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint: &StoredWorkflowEndpoint,
        request: PersistWorkflowEndpointGrantRequest,
    ) -> Result<StoredWorkflowEndpointGrant, SessionStoreError> {
        if !owned_endpoint_matches(endpoint, principal) {
            return Err(SessionStoreError::NotFound(format!(
                "workflow endpoint {} not found",
                endpoint.endpoint_key
            )));
        }
        let now = Utc::now();
        let mut grants = self.workflow_endpoint_grants.lock().await;
        if let Some(grant) = grants.iter_mut().find(|grant| {
            grant.endpoint_id == endpoint.id
                && grant.service_principal_id == request.service_principal_id
        }) {
            grant.operations = request.operations;
            grant.updated_at = now;
            return Ok(grant.clone());
        }
        let grant = StoredWorkflowEndpointGrant {
            id: Uuid::now_v7(),
            endpoint_id: endpoint.id,
            project_id: endpoint.project_id,
            service_principal_id: request.service_principal_id,
            operations: request.operations,
            created_at: now,
            updated_at: now,
        };
        grants.push(grant.clone());
        Ok(grant)
    }

    pub(in crate::session_control) async fn list_workflow_endpoint_grants_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint: &StoredWorkflowEndpoint,
    ) -> Result<Vec<StoredWorkflowEndpointGrant>, SessionStoreError> {
        if !owned_endpoint_matches(endpoint, principal) {
            return Ok(Vec::new());
        }
        let mut grants = self
            .workflow_endpoint_grants
            .lock()
            .await
            .iter()
            .filter(|grant| grant.endpoint_id == endpoint.id)
            .cloned()
            .collect::<Vec<_>>();
        grants.sort_by_key(|grant| grant.created_at);
        Ok(grants)
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_grant(
        &self,
        endpoint_id: Uuid,
        service_principal_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpointGrant>, SessionStoreError> {
        Ok(self
            .workflow_endpoint_grants
            .lock()
            .await
            .iter()
            .find(|grant| {
                grant.endpoint_id == endpoint_id
                    && grant.service_principal_id == service_principal_id
            })
            .cloned())
    }

    pub(in crate::session_control) async fn delete_workflow_endpoint_grant_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint: &StoredWorkflowEndpoint,
        grant_id: Uuid,
    ) -> Result<bool, SessionStoreError> {
        if !owned_endpoint_matches(endpoint, principal) {
            return Ok(false);
        }
        let mut grants = self.workflow_endpoint_grants.lock().await;
        let before = grants.len();
        grants.retain(|grant| !(grant.id == grant_id && grant.endpoint_id == endpoint.id));
        Ok(grants.len() != before)
    }

    pub(in crate::session_control) async fn reserve_workflow_endpoint_invocation(
        &self,
        request: ReserveWorkflowEndpointInvocationRequest,
    ) -> Result<ReserveWorkflowEndpointInvocationResult, SessionStoreError> {
        let mut invocations = self.workflow_endpoint_invocations.lock().await;
        if let Some(existing) = invocations.iter().find(|invocation| {
            invocation.endpoint_id == request.endpoint_id
                && invocation.caller_service_principal_id == request.caller_service_principal_id
                && invocation.idempotency_key == request.idempotency_key
        }) {
            if existing.request_fingerprint != request.request_fingerprint {
                return Err(SessionStoreError::Conflict(
                    "workflow endpoint idempotency key is already bound to a different request"
                        .to_string(),
                ));
            }
            return Ok(ReserveWorkflowEndpointInvocationResult {
                invocation: existing.clone(),
                created: false,
            });
        }
        let now = Utc::now();
        let invocation = StoredWorkflowEndpointInvocation {
            id: Uuid::now_v7(),
            endpoint_id: request.endpoint_id,
            caller_service_principal_id: request.caller_service_principal_id,
            idempotency_key: request.idempotency_key,
            request_fingerprint: request.request_fingerprint,
            run_id: None,
            outcome: None,
            side_effect_state: WorkflowSideEffectState::None,
            created_at: now,
            updated_at: now,
        };
        invocations.push(invocation.clone());
        Ok(ReserveWorkflowEndpointInvocationResult {
            invocation,
            created: true,
        })
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_invocation(
        &self,
        endpoint_id: Uuid,
        invocation_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpointInvocation>, SessionStoreError> {
        Ok(self
            .workflow_endpoint_invocations
            .lock()
            .await
            .iter()
            .find(|invocation| {
                invocation.id == invocation_id && invocation.endpoint_id == endpoint_id
            })
            .cloned())
    }

    pub(in crate::session_control) async fn link_workflow_endpoint_invocation_run(
        &self,
        invocation_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpointInvocation>, SessionStoreError> {
        let mut invocations = self.workflow_endpoint_invocations.lock().await;
        let Some(invocation) = invocations
            .iter_mut()
            .find(|invocation| invocation.id == invocation_id)
        else {
            return Ok(None);
        };
        invocation.run_id = Some(run_id);
        invocation.updated_at = Utc::now();
        Ok(Some(invocation.clone()))
    }

    pub(in crate::session_control) async fn fail_workflow_endpoint_invocation(
        &self,
        invocation_id: Uuid,
        outcome: WorkflowRunOutcome,
        side_effect_state: WorkflowSideEffectState,
    ) -> Result<Option<StoredWorkflowEndpointInvocation>, SessionStoreError> {
        let mut invocations = self.workflow_endpoint_invocations.lock().await;
        let Some(invocation) = invocations
            .iter_mut()
            .find(|invocation| invocation.id == invocation_id)
        else {
            return Ok(None);
        };
        invocation.outcome = Some(outcome);
        invocation.side_effect_state = side_effect_state;
        invocation.updated_at = Utc::now();
        Ok(Some(invocation.clone()))
    }

    pub(in crate::session_control) async fn update_workflow_run_endpoint_evidence(
        &self,
        run_id: Uuid,
        evidence: WorkflowEndpointRunEvidence,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut runs = self.workflow_runs.lock().await;
        let Some(run) = runs.iter_mut().find(|run| run.id == run_id) else {
            return Ok(None);
        };
        run.outcome = Some(evidence.outcome);
        run.side_effect_state = Some(evidence.side_effect_state);
        run.updated_at = Utc::now();
        Ok(Some(run.clone()))
    }
}
