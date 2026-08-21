use std::collections::HashSet;

use super::*;
use crate::workflow_endpoints::{
    MAX_ARTIFACT_RETENTION_SECONDS, MAX_EXECUTION_TIMEOUT_SECONDS, MAX_INLINE_RESULT_BYTES,
    MIN_EXECUTION_TIMEOUT_SECONDS, MIN_INLINE_RESULT_BYTES,
};

impl SessionStore {
    pub(crate) async fn prepare_endpoint_automation_task_transition(
        &self,
        automation_task_id: Uuid,
        mut request: AutomationTaskTransitionRequest,
    ) -> Result<AutomationTaskTransitionRequest, SessionStoreError> {
        if request.state != AutomationTaskState::Succeeded {
            return Ok(request);
        }
        let Some(run) = self
            .get_workflow_run_by_automation_task_id(automation_task_id)
            .await?
        else {
            return Ok(request);
        };
        let Some(endpoint_id) = run.endpoint_id else {
            return Ok(request);
        };
        let Some(endpoint) = self.get_workflow_endpoint_by_id(endpoint_id).await? else {
            return Err(SessionStoreError::NotFound(format!(
                "workflow endpoint {endpoint_id} for run {} not found",
                run.id
            )));
        };
        let instance = request.output.as_ref().unwrap_or(&Value::Null);
        if let Err(violations) =
            crate::workflow_endpoints::validate_schema_instance(&endpoint.output_schema, instance)
        {
            request.state = AutomationTaskState::Failed;
            request.output = None;
            request.error = Some("workflow_endpoint_output_schema_validation_failed".to_string());
            request.event_type = "workflow_endpoint.output_invalid".to_string();
            request.event_message =
                "workflow output did not satisfy the active endpoint schema".to_string();
            request.event_data = Some(serde_json::json!({
                "validation_errors": violations,
            }));
            return Ok(request);
        }
        let output_bytes = serde_json::to_vec(instance).map_err(|error| {
            SessionStoreError::Backend(format!(
                "failed to measure workflow endpoint output: {error}"
            ))
        })?;
        if output_bytes.len() > endpoint.inline_result_max_bytes as usize {
            request.state = AutomationTaskState::Failed;
            request.output = None;
            request.error = Some("workflow_endpoint_output_limit_exceeded".to_string());
            request.event_type = "workflow_endpoint.output_too_large".to_string();
            request.event_message =
                "workflow output exceeded the endpoint inline result limit".to_string();
            request.event_data = Some(serde_json::json!({
                "max_bytes": endpoint.inline_result_max_bytes,
                "actual_bytes": output_bytes.len(),
            }));
        }
        Ok(request)
    }

    pub(crate) async fn prepare_endpoint_workflow_run_transition(
        &self,
        run_id: Uuid,
        request: WorkflowRunTransitionRequest,
    ) -> Result<WorkflowRunTransitionRequest, SessionStoreError> {
        if request.state != WorkflowRunState::Succeeded {
            return Ok(request);
        }
        let Some(run) = self.get_workflow_run_by_id(run_id).await? else {
            return Ok(request);
        };
        let automation_request = AutomationTaskTransitionRequest {
            state: request.state.into(),
            output: request.output.clone(),
            error: request.error.clone(),
            artifact_refs: request.artifact_refs.clone(),
            event_type: "workflow_run.transition".to_string(),
            event_message: request.message.clone().unwrap_or_default(),
            event_data: request.data.clone(),
        };
        let prepared = self
            .prepare_endpoint_automation_task_transition(run.automation_task_id, automation_request)
            .await?;
        Ok(WorkflowRunTransitionRequest {
            state: prepared.state.into(),
            output: prepared.output,
            error: prepared.error,
            artifact_refs: prepared.artifact_refs,
            message: Some(prepared.event_message),
            data: prepared.event_data,
        })
    }

    pub async fn create_workflow_endpoint(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEndpointRequest,
    ) -> Result<StoredWorkflowEndpoint, SessionStoreError> {
        validate_workflow_endpoint_request(&request)?;
        self.validate_workflow_endpoint_binding(principal, &request)
            .await?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_workflow_endpoint(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_workflow_endpoint(principal, request).await
            }
        }
    }

    pub async fn list_workflow_endpoints_for_owner_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEndpoint>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_endpoints_for_owner_project(principal, project_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_endpoints_for_owner_project(principal, project_id)
                    .await
            }
        }
    }

    pub async fn get_workflow_endpoint_for_owner_project_key(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
        endpoint_key: &str,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_workflow_endpoint_for_owner_project_key(
                        principal,
                        project_id,
                        endpoint_key,
                    )
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_workflow_endpoint_for_owner_project_key(
                        principal,
                        project_id,
                        endpoint_key,
                    )
                    .await
            }
        }
    }

    pub async fn get_workflow_endpoint_by_project_key(
        &self,
        project_id: Uuid,
        endpoint_key: &str,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_workflow_endpoint_by_project_key(project_id, endpoint_key)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_workflow_endpoint_by_project_key(project_id, endpoint_key)
                    .await
            }
        }
    }

    pub async fn get_workflow_endpoint_by_id(
        &self,
        endpoint_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_endpoint_by_id(endpoint_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_endpoint_by_id(endpoint_id).await
            }
        }
    }

    pub async fn update_workflow_endpoint_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint_id: Uuid,
        request: PersistWorkflowEndpointRequest,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        validate_workflow_endpoint_request(&request)?;
        let Some(current) = self.get_workflow_endpoint_by_id(endpoint_id).await? else {
            return Ok(None);
        };
        if current.owner_subject != principal.subject || current.owner_issuer != principal.issuer {
            return Ok(None);
        }
        if current.project_id != request.project_id || current.endpoint_key != request.endpoint_key
        {
            return Err(SessionStoreError::Conflict(
                "workflow endpoint project and endpoint key are immutable".to_string(),
            ));
        }
        if current.state == WorkflowEndpointState::Active {
            return Err(SessionStoreError::Conflict(
                "active workflow endpoints must be disabled before update".to_string(),
            ));
        }
        self.validate_workflow_endpoint_binding(principal, &request)
            .await?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .update_workflow_endpoint_for_owner(principal, endpoint_id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .update_workflow_endpoint_for_owner(principal, endpoint_id, request)
                    .await
            }
        }
    }

    pub async fn set_workflow_endpoint_state_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint_id: Uuid,
        state: WorkflowEndpointState,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .set_workflow_endpoint_state_for_owner(principal, endpoint_id, state)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .set_workflow_endpoint_state_for_owner(principal, endpoint_id, state)
                    .await
            }
        }
    }

    pub async fn upsert_workflow_endpoint_grant_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint: &StoredWorkflowEndpoint,
        request: PersistWorkflowEndpointGrantRequest,
    ) -> Result<StoredWorkflowEndpointGrant, SessionStoreError> {
        validate_workflow_endpoint_grant_request(&request)?;
        if endpoint.owner_subject != principal.subject || endpoint.owner_issuer != principal.issuer
        {
            return Err(SessionStoreError::NotFound(format!(
                "workflow endpoint {} not found",
                endpoint.endpoint_key
            )));
        }
        let service_principal = self
            .get_service_principal_for_owner(principal, request.service_principal_id)
            .await?
            .ok_or_else(|| {
                SessionStoreError::NotFound(format!(
                    "service principal {} not found",
                    request.service_principal_id
                ))
            })?;
        if !service_principal
            .allowed_project_ids
            .contains(&endpoint.project_id)
        {
            return invalid("workflow endpoint grant service principal is not allowed for project");
        }
        if request.operations.iter().any(|operation| {
            !service_principal
                .scopes
                .iter()
                .any(|scope| scope == operation.required_scope())
        }) {
            return invalid(
                "workflow endpoint grant operation requires a declared service principal scope",
            );
        }
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .upsert_workflow_endpoint_grant_for_owner(principal, endpoint, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .upsert_workflow_endpoint_grant_for_owner(principal, endpoint, request)
                    .await
            }
        }
    }

    pub async fn list_workflow_endpoint_grants_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint: &StoredWorkflowEndpoint,
    ) -> Result<Vec<StoredWorkflowEndpointGrant>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_endpoint_grants_for_owner(principal, endpoint)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_endpoint_grants_for_owner(principal, endpoint)
                    .await
            }
        }
    }

    pub async fn get_workflow_endpoint_grant(
        &self,
        endpoint_id: Uuid,
        service_principal_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpointGrant>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_workflow_endpoint_grant(endpoint_id, service_principal_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_workflow_endpoint_grant(endpoint_id, service_principal_id)
                    .await
            }
        }
    }

    pub async fn delete_workflow_endpoint_grant_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint: &StoredWorkflowEndpoint,
        grant_id: Uuid,
    ) -> Result<bool, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .delete_workflow_endpoint_grant_for_owner(principal, endpoint, grant_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .delete_workflow_endpoint_grant_for_owner(principal, endpoint, grant_id)
                    .await
            }
        }
    }

    pub async fn reserve_workflow_endpoint_invocation(
        &self,
        request: ReserveWorkflowEndpointInvocationRequest,
    ) -> Result<ReserveWorkflowEndpointInvocationResult, SessionStoreError> {
        validate_workflow_endpoint_invocation_reservation(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.reserve_workflow_endpoint_invocation(request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.reserve_workflow_endpoint_invocation(request).await
            }
        }
    }

    pub async fn get_workflow_endpoint_invocation(
        &self,
        endpoint_id: Uuid,
        invocation_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpointInvocation>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_workflow_endpoint_invocation(endpoint_id, invocation_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_workflow_endpoint_invocation(endpoint_id, invocation_id)
                    .await
            }
        }
    }

    pub async fn link_workflow_endpoint_invocation_run(
        &self,
        invocation_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpointInvocation>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .link_workflow_endpoint_invocation_run(invocation_id, run_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .link_workflow_endpoint_invocation_run(invocation_id, run_id)
                    .await
            }
        }
    }

    pub async fn fail_workflow_endpoint_invocation(
        &self,
        invocation_id: Uuid,
        outcome: WorkflowRunOutcome,
        side_effect_state: WorkflowSideEffectState,
    ) -> Result<Option<StoredWorkflowEndpointInvocation>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .fail_workflow_endpoint_invocation(invocation_id, outcome, side_effect_state)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .fail_workflow_endpoint_invocation(invocation_id, outcome, side_effect_state)
                    .await
            }
        }
    }

    pub(crate) async fn persist_workflow_run_endpoint_evidence(
        &self,
        run: &StoredWorkflowRun,
        transition_data: Option<&Value>,
    ) -> Result<StoredWorkflowRun, SessionStoreError> {
        let Some(evidence) =
            crate::workflow_endpoints::derive_terminal_evidence(run, transition_data)
        else {
            return Ok(run.clone());
        };
        if run.outcome.as_ref() == Some(&evidence.outcome)
            && run.side_effect_state == Some(evidence.side_effect_state)
        {
            return Ok(run.clone());
        }
        let updated = match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .update_workflow_run_endpoint_evidence(run.id, evidence)
                    .await?
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .update_workflow_run_endpoint_evidence(run.id, evidence)
                    .await?
            }
        };
        Ok(updated.unwrap_or_else(|| run.clone()))
    }

    async fn validate_workflow_endpoint_binding(
        &self,
        principal: &AuthenticatedPrincipal,
        request: &PersistWorkflowEndpointRequest,
    ) -> Result<(), SessionStoreError> {
        if self
            .get_project_for_owner(principal, request.project_id)
            .await?
            .is_none()
            || self
                .get_workflow_definition_for_owner(principal, request.workflow_definition_id)
                .await?
                .is_none()
        {
            return Err(SessionStoreError::NotFound(
                "workflow endpoint project or workflow definition was not found".to_string(),
            ));
        }
        let version = self
            .get_workflow_definition_version_for_owner(
                principal,
                request.workflow_definition_id,
                &request.workflow_version,
            )
            .await?;
        if version.is_none_or(|version| version.id != request.workflow_definition_version_id) {
            return Err(SessionStoreError::NotFound(
                "workflow endpoint immutable workflow version was not found".to_string(),
            ));
        }
        Ok(())
    }
}

fn validate_workflow_endpoint_request(
    request: &PersistWorkflowEndpointRequest,
) -> Result<(), SessionStoreError> {
    if request.project_id.is_nil() {
        return invalid("workflow endpoint project_id must not be nil");
    }
    if request.endpoint_key.is_empty()
        || request.endpoint_key.len() > 64
        || !request
            .endpoint_key
            .bytes()
            .enumerate()
            .all(|(index, byte)| {
                byte.is_ascii_lowercase()
                    || byte.is_ascii_digit()
                    || (byte == b'-' && index > 0 && index + 1 < request.endpoint_key.len())
            })
    {
        return invalid(
            "workflow endpoint key must be 1-64 lowercase letters, digits, or interior hyphens",
        );
    }
    if request.purpose.trim().is_empty() || request.purpose.len() > 512 {
        return invalid("workflow endpoint purpose must contain 1-512 characters");
    }
    if request.workflow_definition_id.is_nil()
        || request.workflow_definition_version_id.is_nil()
        || request.workflow_version.trim().is_empty()
    {
        return invalid("workflow endpoint must bind a workflow definition and version");
    }
    if !(MIN_EXECUTION_TIMEOUT_SECONDS..=MAX_EXECUTION_TIMEOUT_SECONDS)
        .contains(&request.execution_timeout_seconds)
    {
        return invalid("workflow endpoint execution timeout is outside the supported range");
    }
    if !(MIN_INLINE_RESULT_BYTES..=MAX_INLINE_RESULT_BYTES)
        .contains(&request.inline_result_max_bytes)
    {
        return invalid("workflow endpoint inline result limit is outside the supported range");
    }
    if request.artifact_behavior.mode != "authorized_references"
        || request.artifact_behavior.retention_seconds == 0
        || request.artifact_behavior.retention_seconds > MAX_ARTIFACT_RETENTION_SECONDS
    {
        return invalid("workflow endpoint artifact behavior is invalid");
    }
    Ok(())
}

fn validate_workflow_endpoint_grant_request(
    request: &PersistWorkflowEndpointGrantRequest,
) -> Result<(), SessionStoreError> {
    if request.service_principal_id.is_nil() {
        return invalid("workflow endpoint grant service_principal_id must not be nil");
    }
    if request.operations.is_empty() {
        return invalid("workflow endpoint grant must contain at least one operation");
    }
    let unique = request.operations.iter().copied().collect::<HashSet<_>>();
    if unique.len() != request.operations.len() {
        return invalid("workflow endpoint grant operations must not contain duplicates");
    }
    Ok(())
}

fn validate_workflow_endpoint_invocation_reservation(
    request: &ReserveWorkflowEndpointInvocationRequest,
) -> Result<(), SessionStoreError> {
    if request.endpoint_id.is_nil() || request.caller_service_principal_id.is_nil() {
        return invalid("workflow endpoint invocation ids must not be nil");
    }
    if request.idempotency_key.trim().is_empty() || request.idempotency_key.len() > 128 {
        return invalid("workflow endpoint idempotency key must contain 1-128 characters");
    }
    if request.request_fingerprint.len() != 64
        || !request
            .request_fingerprint
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return invalid("workflow endpoint request fingerprint must be a SHA-256 hex digest");
    }
    Ok(())
}

fn invalid<T>(message: &str) -> Result<T, SessionStoreError> {
    Err(SessionStoreError::InvalidRequest(message.to_string()))
}
