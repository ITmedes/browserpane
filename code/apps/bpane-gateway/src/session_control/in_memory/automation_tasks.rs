use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_automation_task(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistAutomationTaskRequest,
    ) -> Result<StoredAutomationTask, SessionStoreError> {
        let now = Utc::now();
        let task = StoredAutomationTask {
            id: Uuid::now_v7(),
            display_name: request.display_name,
            executor: request.executor,
            state: AutomationTaskState::Pending,
            session_id: request.session_id,
            session_source: request.session_source,
            input: request.input,
            output: None,
            error: None,
            artifact_refs: Vec::new(),
            labels: request.labels,
            cancel_requested_at: None,
            started_at: None,
            completed_at: None,
            created_at: now,
            updated_at: now,
        };
        let event = StoredAutomationTaskEvent {
            id: Uuid::now_v7(),
            task_id: task.id,
            event_type: "automation_task.created".to_string(),
            message: "automation task created".to_string(),
            data: Some(serde_json::json!({
                "session_id": task.session_id,
                "session_source": task.session_source.as_str(),
                "executor": task.executor,
                "owner_subject": principal.subject,
                "owner_issuer": principal.issuer,
            })),
            created_at: now,
        };
        self.automation_tasks.lock().await.push(task.clone());
        self.automation_task_events.lock().await.push(event);
        Ok(task)
    }

    pub(in crate::session_control) async fn list_automation_tasks_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredAutomationTask>, SessionStoreError> {
        let sessions = self.sessions.lock().await;
        let visible_session_ids = sessions
            .iter()
            .filter(|session| task_visible_to_principal(session, principal))
            .map(|session| session.id)
            .collect::<Vec<_>>();
        drop(sessions);

        let mut tasks = self
            .automation_tasks
            .lock()
            .await
            .iter()
            .filter(|task| visible_session_ids.contains(&task.session_id))
            .cloned()
            .collect::<Vec<_>>();
        tasks.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(tasks)
    }

    pub(in crate::session_control) async fn get_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let Some(task) = self
            .automation_tasks
            .lock()
            .await
            .iter()
            .find(|task| task.id == id)
            .cloned()
        else {
            return Ok(None);
        };
        let Some(session) = self.get_session_by_id(task.session_id).await? else {
            return Ok(None);
        };
        if !task_visible_to_principal(&session, principal) {
            return Ok(None);
        }
        Ok(Some(task))
    }

    pub(in crate::session_control) async fn get_automation_task_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        Ok(self
            .automation_tasks
            .lock()
            .await
            .iter()
            .find(|task| task.id == id)
            .cloned())
    }

    pub(in crate::session_control) async fn cancel_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let visible = self.get_automation_task_for_owner(principal, id).await?;
        let Some(visible) = visible else {
            return Ok(None);
        };
        let cancellation_plan = plan_automation_task_cancellation(&visible, Utc::now())
            .map_err(|error| SessionStoreError::Conflict(error.to_string()))?;

        let mut tasks = self.automation_tasks.lock().await;
        let Some(task) = tasks.iter_mut().find(|task| task.id == id) else {
            return Ok(None);
        };
        let now = cancellation_plan.task_updated_at;
        task.state = cancellation_plan.task_state;
        task.cancel_requested_at = cancellation_plan.task_cancel_requested_at;
        task.started_at = cancellation_plan.task_started_at;
        task.completed_at = cancellation_plan.task_completed_at;
        task.updated_at = cancellation_plan.task_updated_at;
        let task = task.clone();
        drop(tasks);

        let workflow_run_id = if let Some(run) = self
            .workflow_runs
            .lock()
            .await
            .iter_mut()
            .find(|run| run.automation_task_id == id)
        {
            sync_workflow_run_with_task(run, &task);
            Some(run.id)
        } else {
            None
        };

        self.automation_task_events
            .lock()
            .await
            .push(StoredAutomationTaskEvent {
                id: Uuid::now_v7(),
                task_id: id,
                event_type: cancellation_plan.task_event_type,
                message: cancellation_plan.task_event_message,
                data: cancellation_plan.task_event_data,
                created_at: now,
            });
        self.automation_task_logs
            .lock()
            .await
            .push(StoredAutomationTaskLog {
                id: Uuid::now_v7(),
                task_id: id,
                stream: cancellation_plan.task_log_stream,
                message: cancellation_plan.task_log_message,
                created_at: now,
            });
        if let Some(run_id) = workflow_run_id {
            let event = StoredWorkflowRunEvent {
                id: Uuid::now_v7(),
                run_id,
                event_type: cancellation_plan.run_event_type,
                message: cancellation_plan.run_event_message,
                data: cancellation_plan.run_event_data,
                created_at: now,
            };
            self.workflow_run_events.lock().await.push(event.clone());
            if let Some(run) = self
                .workflow_runs
                .lock()
                .await
                .iter()
                .find(|run| run.id == run_id)
                .cloned()
            {
                self.queue_workflow_event_deliveries_for_run_event(&run, &event)
                    .await;
            }
            self.workflow_run_logs
                .lock()
                .await
                .push(StoredWorkflowRunLog {
                    id: Uuid::now_v7(),
                    run_id,
                    stream: cancellation_plan.run_log_stream,
                    message: cancellation_plan.run_log_message,
                    created_at: now,
                });
        }
        Ok(Some(task))
    }

    pub(in crate::session_control) async fn list_automation_task_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskEvent>, SessionStoreError> {
        if self
            .get_automation_task_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut events = self
            .automation_task_events
            .lock()
            .await
            .iter()
            .filter(|event| event.task_id == id)
            .cloned()
            .collect::<Vec<_>>();
        events.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(events)
    }

    pub(in crate::session_control) async fn list_automation_task_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskLog>, SessionStoreError> {
        if self
            .get_automation_task_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }

        let mut logs = self
            .automation_task_logs
            .lock()
            .await
            .iter()
            .filter(|log| log.task_id == id)
            .cloned()
            .collect::<Vec<_>>();
        logs.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(logs)
    }

    pub(in crate::session_control) async fn transition_automation_task(
        &self,
        id: Uuid,
        request: AutomationTaskTransitionRequest,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        let mut tasks = self.automation_tasks.lock().await;
        let Some(task) = tasks.iter_mut().find(|task| task.id == id) else {
            return Ok(None);
        };
        let current_task = task.clone();
        let transition_plan = plan_automation_task_transition(&current_task, &request, Utc::now())
            .map_err(|error| SessionStoreError::Conflict(error.to_string()))?;
        let now = transition_plan.task_updated_at;
        task.state = transition_plan.task_state;
        task.output = transition_plan.task_output.clone();
        task.error = transition_plan.task_error.clone();
        task.artifact_refs = transition_plan.task_artifact_refs.clone();
        task.started_at = transition_plan.task_started_at;
        task.completed_at = transition_plan.task_completed_at;
        task.updated_at = transition_plan.task_updated_at;
        let task = task.clone();
        drop(tasks);

        if let Some(run) = self
            .workflow_runs
            .lock()
            .await
            .iter_mut()
            .find(|run| run.automation_task_id == id)
        {
            sync_workflow_run_with_task(run, &task);
        }

        self.automation_task_events
            .lock()
            .await
            .push(StoredAutomationTaskEvent {
                id: Uuid::now_v7(),
                task_id: id,
                event_type: transition_plan.task_event_type,
                message: transition_plan.task_event_message,
                data: transition_plan.task_event_data,
                created_at: now,
            });
        Ok(Some(task))
    }

    pub(in crate::session_control) async fn append_automation_task_log(
        &self,
        id: Uuid,
        stream: AutomationTaskLogStream,
        message: String,
    ) -> Result<Option<StoredAutomationTaskLog>, SessionStoreError> {
        let tasks = self.automation_tasks.lock().await;
        if !tasks.iter().any(|task| task.id == id) {
            return Ok(None);
        }
        drop(tasks);

        let log = StoredAutomationTaskLog {
            id: Uuid::now_v7(),
            task_id: id,
            stream,
            message,
            created_at: Utc::now(),
        };
        self.automation_task_logs.lock().await.push(log.clone());
        Ok(Some(log))
    }
}
