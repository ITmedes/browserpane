use super::super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn append_workflow_run_event_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: Utc::now(),
        };
        self.workflow_run_events.lock().await.push(event.clone());
        let mut updated_run = None;
        {
            let mut runs = self.workflow_runs.lock().await;
            if let Some(run) = runs.iter_mut().find(|run| run.id == id) {
                run.updated_at = event.created_at;
                updated_run = Some(run.clone());
            }
        }
        if let Some(run) = updated_run.as_ref() {
            self.queue_workflow_event_deliveries_for_run_event(run, &event)
                .await;
        }
        Ok(Some(event))
    }

    pub(in crate::session_control) async fn append_workflow_run_event(
        &self,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        if self.get_workflow_run_by_id(id).await?.is_none() {
            return Ok(None);
        }
        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: Utc::now(),
        };
        self.workflow_run_events.lock().await.push(event.clone());
        let mut updated_run = None;
        {
            let mut runs = self.workflow_runs.lock().await;
            if let Some(run) = runs.iter_mut().find(|run| run.id == id) {
                run.updated_at = event.created_at;
                updated_run = Some(run.clone());
            }
        }
        if let Some(run) = updated_run.as_ref() {
            self.queue_workflow_event_deliveries_for_run_event(run, &event)
                .await;
        }
        Ok(Some(event))
    }

    pub(in crate::session_control) async fn transition_workflow_run(
        &self,
        id: Uuid,
        request: WorkflowRunTransitionRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let run = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .find(|run| run.id == id)
            .cloned();
        let Some(run) = run else {
            return Ok(None);
        };

        let now = Utc::now();

        let plan;
        let task = {
            let mut tasks = self.automation_tasks.lock().await;
            let Some(task) = tasks
                .iter_mut()
                .find(|task| task.id == run.automation_task_id)
            else {
                return Err(SessionStoreError::NotFound(format!(
                    "automation task {} for workflow run {} not found",
                    run.automation_task_id, id
                )));
            };
            let current_task = task.clone();
            let transition_plan = plan_workflow_run_transition(&current_task, &request, now)
                .map_err(|error| SessionStoreError::Conflict(error.to_string()))?;
            task.state = transition_plan.task_state;
            task.output = transition_plan.task_output.clone();
            task.error = transition_plan.task_error.clone();
            task.artifact_refs = transition_plan.task_artifact_refs.clone();
            task.started_at = transition_plan.task_started_at;
            task.completed_at = transition_plan.task_completed_at;
            task.updated_at = transition_plan.task_updated_at;
            plan = transition_plan;
            task.clone()
        };
        self.automation_task_events
            .lock()
            .await
            .push(StoredAutomationTaskEvent {
                id: Uuid::now_v7(),
                task_id: task.id,
                event_type: plan.task_event_type.clone(),
                message: plan.task_event_message.clone(),
                data: plan.task_event_data.clone(),
                created_at: now,
            });

        let run = {
            let mut runs = self.workflow_runs.lock().await;
            let Some(run) = runs.iter_mut().find(|run| run.id == id) else {
                return Ok(None);
            };
            run.state = plan.run_state;
            run.output = plan.run_output.clone();
            run.error = plan.run_error.clone();
            run.artifact_refs = plan.run_artifact_refs.clone();
            run.started_at = plan.run_started_at;
            run.completed_at = plan.run_completed_at;
            run.updated_at = plan.run_updated_at;
            run.clone()
        };

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: plan.run_event_type,
            message: plan.run_event_message,
            data: plan.run_event_data,
            created_at: now,
        };
        self.workflow_run_events.lock().await.push(event.clone());
        self.queue_workflow_event_deliveries_for_run_event(&run, &event)
            .await;

        Ok(Some(run))
    }

    pub(in crate::session_control) async fn reconcile_workflow_run_from_task(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let run = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .find(|run| run.id == id)
            .cloned();
        let Some(current_run) = run else {
            return Ok(None);
        };

        let task = self
            .automation_tasks
            .lock()
            .await
            .iter()
            .find(|task| task.id == current_run.automation_task_id)
            .cloned()
            .ok_or_else(|| {
                SessionStoreError::NotFound(format!(
                    "automation task {} for workflow run {} not found",
                    current_run.automation_task_id, id
                ))
            })?;
        if !task.state.is_terminal() {
            return Ok(Some(current_run));
        }

        let now = Utc::now();
        let (decision, plan) = plan_workflow_run_reconciliation(&current_run, &task, now);
        match decision {
            WorkflowRunReconciliationDecision::NotTerminal
            | WorkflowRunReconciliationDecision::Unchanged => return Ok(Some(current_run)),
            WorkflowRunReconciliationDecision::Update => {}
        }
        let plan = plan.expect("workflow run reconciliation update plan must exist");
        let run = {
            let mut runs = self.workflow_runs.lock().await;
            let Some(run) = runs.iter_mut().find(|run| run.id == id) else {
                return Ok(None);
            };
            run.state = plan.run_state;
            run.output = plan.run_output.clone();
            run.error = plan.run_error.clone();
            run.artifact_refs = plan.run_artifact_refs.clone();
            run.started_at = plan.run_started_at;
            run.completed_at = plan.run_completed_at;
            run.updated_at = plan.run_updated_at;
            run.clone()
        };

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: plan.run_event_type,
            message: plan.run_event_message,
            data: plan.run_event_data,
            created_at: now,
        };
        self.workflow_run_events.lock().await.push(event.clone());
        self.queue_workflow_event_deliveries_for_run_event(&run, &event)
            .await;
        Ok(Some(run))
    }

    pub(in crate::session_control) async fn append_workflow_run_log(
        &self,
        id: Uuid,
        request: PersistWorkflowRunLogRequest,
    ) -> Result<Option<StoredWorkflowRunLog>, SessionStoreError> {
        let mut runs = self.workflow_runs.lock().await;
        let Some(run) = runs.iter_mut().find(|run| run.id == id) else {
            return Ok(None);
        };

        let log = StoredWorkflowRunLog {
            id: Uuid::now_v7(),
            run_id: id,
            stream: request.stream,
            message: request.message,
            created_at: Utc::now(),
        };
        run.updated_at = log.created_at;
        drop(runs);

        self.workflow_run_logs.lock().await.push(log.clone());
        Ok(Some(log))
    }

    pub(in crate::session_control) async fn append_workflow_run_produced_file(
        &self,
        id: Uuid,
        request: PersistWorkflowRunProducedFileRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let now = Utc::now();
        let produced_file = WorkflowRunProducedFile {
            workspace_id: request.workspace_id,
            file_id: request.file_id,
            file_name: request.file_name,
            media_type: request.media_type,
            byte_count: request.byte_count,
            sha256_hex: request.sha256_hex,
            provenance: request.provenance,
            artifact_ref: request.artifact_ref,
            created_at: now,
        };

        let mut runs = self.workflow_runs.lock().await;
        let Some(run) = runs.iter_mut().find(|run| run.id == id) else {
            return Ok(None);
        };
        if run
            .produced_files
            .iter()
            .any(|file| file.file_id == produced_file.file_id)
        {
            return Err(SessionStoreError::Conflict(format!(
                "workflow run {id} already contains produced file {}",
                produced_file.file_id
            )));
        }
        run.produced_files.push(produced_file.clone());
        run.updated_at = now;
        let updated = run.clone();
        drop(runs);

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: "workflow_run.produced_file_added".to_string(),
            message: format!(
                "workflow run produced file {} stored in workspace {}",
                produced_file.file_id, produced_file.workspace_id
            ),
            data: Some(serde_json::json!({
                "workspace_id": produced_file.workspace_id,
                "file_id": produced_file.file_id,
                "file_name": produced_file.file_name,
            })),
            created_at: now,
        };
        self.workflow_run_events.lock().await.push(event.clone());
        self.queue_workflow_event_deliveries_for_run_event(&updated, &event)
            .await;

        Ok(Some(updated))
    }
}
