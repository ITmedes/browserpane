use super::super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_workflow_run(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowRunRequest,
    ) -> Result<CreateWorkflowRunResult, SessionStoreError> {
        let Some(task) = self
            .get_automation_task_for_owner(principal, request.automation_task_id)
            .await?
        else {
            return Err(SessionStoreError::NotFound(format!(
                "automation task {} not found",
                request.automation_task_id
            )));
        };
        if task.session_id != request.session_id {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run session_id must match the bound automation task session".to_string(),
            ));
        }
        if self
            .get_workflow_definition_for_owner(principal, request.workflow_definition_id)
            .await?
            .is_none()
        {
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition {} not found",
                request.workflow_definition_id
            )));
        }
        let Some(version) = self
            .workflow_definition_versions
            .lock()
            .await
            .iter()
            .find(|version| version.id == request.workflow_definition_version_id)
            .cloned()
        else {
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition version {} not found",
                request.workflow_definition_version_id
            )));
        };
        if version.workflow_definition_id != request.workflow_definition_id {
            return Err(SessionStoreError::InvalidRequest(
                "workflow run version must belong to the requested workflow definition".to_string(),
            ));
        }

        if let Some(client_request_id) = request.client_request_id.as_deref() {
            let existing_run = {
                let runs = self.workflow_runs.lock().await;
                runs.iter()
                    .find(|run| {
                        run.owner_subject == principal.subject
                            && run.owner_issuer == principal.issuer
                            && run.client_request_id.as_deref() == Some(client_request_id)
                    })
                    .cloned()
            };
            if let Some(existing_run) = existing_run {
                if existing_run.create_request_fingerprint == request.create_request_fingerprint {
                    return Ok(CreateWorkflowRunResult {
                        run: existing_run,
                        created: false,
                    });
                }
                return Err(SessionStoreError::Conflict(format!(
                    "workflow run client_request_id {} is already bound to a different request",
                    client_request_id
                )));
            }
        }

        let now = Utc::now();
        let run = StoredWorkflowRun {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            workflow_definition_id: request.workflow_definition_id,
            workflow_definition_version_id: request.workflow_definition_version_id,
            workflow_version: request.workflow_version.clone(),
            session_id: request.session_id,
            automation_task_id: request.automation_task_id,
            source_system: request.source_system.clone(),
            source_reference: request.source_reference.clone(),
            client_request_id: request.client_request_id.clone(),
            create_request_fingerprint: request.create_request_fingerprint.clone(),
            source_snapshot: request.source_snapshot,
            extensions: request.extensions,
            credential_bindings: request.credential_bindings,
            workspace_inputs: request.workspace_inputs,
            produced_files: Vec::new(),
            state: WorkflowRunState::Pending,
            input: request.input,
            output: None,
            error: None,
            artifact_refs: Vec::new(),
            labels: request.labels,
            started_at: None,
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        self.workflow_runs.lock().await.push(run.clone());
        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: run.id,
            event_type: "workflow_run.created".to_string(),
            message: "workflow run created".to_string(),
            data: Some(serde_json::json!({
                "workflow_definition_id": run.workflow_definition_id,
                "workflow_definition_version_id": run.workflow_definition_version_id,
                "workflow_version": run.workflow_version,
                "automation_task_id": run.automation_task_id,
                "session_id": run.session_id,
                "source_system": run.source_system.clone(),
                "source_reference": run.source_reference.clone(),
                "client_request_id": run.client_request_id.clone(),
            })),
            created_at: now,
        };
        self.workflow_run_events.lock().await.push(event.clone());
        self.queue_workflow_event_deliveries_for_run_event(&run, &event)
            .await;
        Ok(CreateWorkflowRunResult { run, created: true })
    }

    pub(in crate::session_control) async fn get_workflow_run_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let Some(run) = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .find(|run| run.id == id)
            .cloned()
        else {
            return Ok(None);
        };
        let Some(task) = self
            .get_automation_task_for_owner(principal, run.automation_task_id)
            .await?
        else {
            return Ok(None);
        };
        if task.session_id != run.session_id {
            return Ok(None);
        }
        Ok(Some(run))
    }

    pub(in crate::session_control) async fn get_workflow_run_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        Ok(self
            .workflow_runs
            .lock()
            .await
            .iter()
            .find(|run| run.id == id)
            .cloned())
    }

    pub(in crate::session_control) async fn list_dispatchable_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        let mut runs = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter(|run| {
                matches!(
                    run.state,
                    WorkflowRunState::Pending | WorkflowRunState::Queued
                )
            })
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(runs)
    }

    pub(in crate::session_control) async fn find_workflow_run_by_client_request_id_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        client_request_id: &str,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let runs = self.workflow_runs.lock().await.clone();
        for run in runs {
            if run.owner_subject == principal.subject
                && run.owner_issuer == principal.issuer
                && run.client_request_id.as_deref() == Some(client_request_id)
            {
                return Ok(Some(run));
            }
        }
        Ok(None)
    }

    pub(in crate::session_control) async fn list_workflow_run_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let mut events = self
            .workflow_run_events
            .lock()
            .await
            .iter()
            .filter(|event| event.run_id == id)
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(events)
    }

    pub(in crate::session_control) async fn list_workflow_run_events(
        &self,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        let mut events = self
            .workflow_run_events
            .lock()
            .await
            .iter()
            .filter(|event| event.run_id == id)
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(events)
    }

    pub(in crate::session_control) async fn list_workflow_run_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunLog>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut logs = self
            .workflow_run_logs
            .lock()
            .await
            .iter()
            .filter(|log| log.run_id == id)
            .cloned()
            .collect::<Vec<_>>();
        logs.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(logs)
    }

    pub(in crate::session_control) async fn list_awaiting_input_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        let mut runs = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter(|run| run.state == WorkflowRunState::AwaitingInput)
            .cloned()
            .collect::<Vec<_>>();
        runs.sort_by(|left, right| {
            left.updated_at
                .cmp(&right.updated_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(runs)
    }
}
