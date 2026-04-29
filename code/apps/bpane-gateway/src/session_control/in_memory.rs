use super::*;
use std::ops::Deref;

mod automation_tasks;
mod recordings;
mod runtime_assignments;
mod sessions;
mod state;
mod workflow_definitions;

use state::*;

pub(super) struct InMemorySessionStore {
    state: InMemoryStoreState,
    config: SessionStoreConfig,
}

impl Deref for InMemorySessionStore {
    type Target = InMemoryStoreState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl InMemorySessionStore {
    pub(super) fn new(config: SessionStoreConfig) -> Self {
        Self {
            state: InMemoryStoreState::new(),
            config,
        }
    }

    pub(super) async fn queue_workflow_event_deliveries_for_run_event(
        &self,
        run: &StoredWorkflowRun,
        event: &StoredWorkflowRunEvent,
    ) {
        let subscriptions = self.workflow_event_subscriptions.lock().await.clone();
        let planned_deliveries = plan_workflow_event_deliveries(&subscriptions, run, event);
        self.workflow_event_deliveries
            .lock()
            .await
            .extend(planned_deliveries);
    }

    pub(super) async fn create_workflow_run(
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

    pub(super) async fn get_workflow_run_for_owner(
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

    pub(super) async fn get_workflow_run_by_id(
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

    pub(super) async fn list_dispatchable_workflow_runs(
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

    pub(super) async fn find_workflow_run_by_client_request_id_for_owner(
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

    pub(super) async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        let now = Utc::now();
        let subscription = StoredWorkflowEventSubscription {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            target_url: request.target_url,
            event_types: request.event_types,
            signing_secret: request.signing_secret,
            created_at: now,
            updated_at: now,
        };
        self.workflow_event_subscriptions
            .lock()
            .await
            .push(subscription.clone());
        Ok(subscription)
    }

    pub(super) async fn list_workflow_event_subscriptions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowEventSubscription>, SessionStoreError> {
        let mut subscriptions = self
            .workflow_event_subscriptions
            .lock()
            .await
            .iter()
            .filter(|subscription| {
                subscription.owner_subject == principal.subject
                    && subscription.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        subscriptions.sort_by(|left, right| {
            right
                .created_at
                .cmp(&left.created_at)
                .then_with(|| right.id.cmp(&left.id))
        });
        Ok(subscriptions)
    }

    pub(super) async fn get_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        Ok(self
            .workflow_event_subscriptions
            .lock()
            .await
            .iter()
            .find(|subscription| {
                subscription.id == id
                    && subscription.owner_subject == principal.subject
                    && subscription.owner_issuer == principal.issuer
            })
            .cloned())
    }

    pub(super) async fn delete_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        let mut subscriptions = self.workflow_event_subscriptions.lock().await;
        let Some(index) = subscriptions.iter().position(|subscription| {
            subscription.id == id
                && subscription.owner_subject == principal.subject
                && subscription.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };
        let removed = subscriptions.remove(index);
        drop(subscriptions);

        let delivery_ids = {
            let mut deliveries = self.workflow_event_deliveries.lock().await;
            let delivery_ids = deliveries
                .iter()
                .filter(|delivery| delivery.subscription_id == id)
                .map(|delivery| delivery.id)
                .collect::<Vec<_>>();
            deliveries.retain(|delivery| delivery.subscription_id != id);
            delivery_ids
        };
        self.workflow_event_delivery_attempts
            .lock()
            .await
            .retain(|attempt| !delivery_ids.contains(&attempt.delivery_id));
        Ok(Some(removed))
    }

    pub(super) async fn list_workflow_event_deliveries_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        if self
            .get_workflow_event_subscription_for_owner(principal, subscription_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let mut deliveries = self
            .workflow_event_deliveries
            .lock()
            .await
            .iter()
            .filter(|delivery| delivery.subscription_id == subscription_id)
            .cloned()
            .collect::<Vec<_>>();
        deliveries.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.event_id.cmp(&right.event_id))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(deliveries)
    }

    pub(super) async fn list_workflow_event_delivery_attempts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDeliveryAttempt>, SessionStoreError> {
        let deliveries = self
            .list_workflow_event_deliveries_for_owner(principal, subscription_id)
            .await?;
        let delivery_ids = deliveries
            .into_iter()
            .map(|delivery| delivery.id)
            .collect::<Vec<_>>();
        let mut attempts = self
            .workflow_event_delivery_attempts
            .lock()
            .await
            .iter()
            .filter(|attempt| delivery_ids.contains(&attempt.delivery_id))
            .cloned()
            .collect::<Vec<_>>();
        attempts.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(attempts)
    }

    pub(super) async fn requeue_inflight_workflow_event_deliveries(
        &self,
    ) -> Result<(), SessionStoreError> {
        let now = Utc::now();
        for delivery in self.workflow_event_deliveries.lock().await.iter_mut() {
            if delivery.state == WorkflowEventDeliveryState::Delivering {
                delivery.state = WorkflowEventDeliveryState::Pending;
                delivery.next_attempt_at = Some(now);
                delivery.updated_at = now;
            }
        }
        Ok(())
    }

    pub(super) async fn claim_due_workflow_event_deliveries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        let mut deliveries = self.workflow_event_deliveries.lock().await;
        let mut due_indexes = deliveries
            .iter()
            .enumerate()
            .filter(|(_, delivery)| {
                delivery.state == WorkflowEventDeliveryState::Pending
                    && delivery
                        .next_attempt_at
                        .map(|value| value <= now)
                        .unwrap_or(true)
            })
            .map(|(index, delivery)| (index, delivery.created_at, delivery.event_id, delivery.id))
            .collect::<Vec<_>>();
        due_indexes.sort_by(|left, right| {
            left.1
                .cmp(&right.1)
                .then_with(|| left.2.cmp(&right.2))
                .then_with(|| left.3.cmp(&right.3))
        });
        let mut due_indexes = due_indexes
            .into_iter()
            .map(|(index, _, _, _)| index)
            .take(limit)
            .collect::<Vec<_>>();
        let mut claimed = Vec::with_capacity(due_indexes.len());
        for index in due_indexes.drain(..) {
            if let Some(delivery) = deliveries.get_mut(index) {
                delivery.state = WorkflowEventDeliveryState::Delivering;
                delivery.updated_at = now;
                claimed.push(delivery.clone());
            }
        }
        Ok(claimed)
    }

    pub(super) async fn record_workflow_event_delivery_attempt(
        &self,
        delivery_id: Uuid,
        request: RecordWorkflowEventDeliveryAttemptRequest,
    ) -> Result<Option<StoredWorkflowEventDelivery>, SessionStoreError> {
        let now = request.attempted_at;
        let mut deliveries = self.workflow_event_deliveries.lock().await;
        let Some(delivery) = deliveries
            .iter_mut()
            .find(|delivery| delivery.id == delivery_id)
        else {
            return Ok(None);
        };
        delivery.state = request.state;
        delivery.attempt_count = request.attempt_number;
        delivery.next_attempt_at = request.next_attempt_at;
        delivery.last_attempt_at = Some(now);
        delivery.delivered_at = request.delivered_at;
        delivery.last_response_status = request.response_status;
        delivery.last_error = request.error.clone();
        delivery.updated_at = now;
        let updated = delivery.clone();
        drop(deliveries);

        self.workflow_event_delivery_attempts.lock().await.push(
            StoredWorkflowEventDeliveryAttempt {
                id: Uuid::now_v7(),
                delivery_id,
                attempt_number: request.attempt_number,
                response_status: request.response_status,
                error: request.error,
                created_at: now,
            },
        );
        Ok(Some(updated))
    }

    pub(super) async fn list_workflow_run_events_for_owner(
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

    pub(super) async fn list_workflow_run_events(
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

    pub(super) async fn list_workflow_run_logs_for_owner(
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

    pub(super) async fn append_workflow_run_event_for_owner(
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

    pub(super) async fn append_workflow_run_event(
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

    pub(super) async fn transition_workflow_run(
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

    pub(super) async fn list_awaiting_input_workflow_runs(
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

    pub(super) async fn reconcile_workflow_run_from_task(
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

    pub(super) async fn append_workflow_run_log(
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

    pub(super) async fn append_workflow_run_produced_file(
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

    pub(super) async fn list_workflow_run_log_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunLogRetentionCandidate>, SessionStoreError> {
        let task_logs = self.automation_task_logs.lock().await;
        let run_logs = self.workflow_run_logs.lock().await;
        let mut candidates = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter_map(|run| {
                let completed_at = run.completed_at?;
                if completed_at + retention > now {
                    return None;
                }
                let has_logs = run_logs.iter().any(|log| log.run_id == run.id)
                    || task_logs
                        .iter()
                        .any(|log| log.task_id == run.automation_task_id);
                if !has_logs {
                    return None;
                }
                Some(WorkflowRunLogRetentionCandidate {
                    run_id: run.id,
                    automation_task_id: run.automation_task_id,
                    session_id: run.session_id,
                    expires_at: completed_at + retention,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.expires_at
                .cmp(&right.expires_at)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        Ok(candidates)
    }

    pub(super) async fn delete_workflow_run_logs(
        &self,
        run_id: Uuid,
        automation_task_id: Uuid,
    ) -> Result<usize, SessionStoreError> {
        let mut deleted = 0usize;
        {
            let mut logs = self.workflow_run_logs.lock().await;
            let before = logs.len();
            logs.retain(|log| log.run_id != run_id);
            deleted += before - logs.len();
        }
        {
            let mut logs = self.automation_task_logs.lock().await;
            let before = logs.len();
            logs.retain(|log| log.task_id != automation_task_id);
            deleted += before - logs.len();
        }
        if let Some(run) = self
            .workflow_runs
            .lock()
            .await
            .iter_mut()
            .find(|run| run.id == run_id)
        {
            run.updated_at = Utc::now();
        }
        Ok(deleted)
    }

    pub(super) async fn list_workflow_run_output_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunOutputRetentionCandidate>, SessionStoreError> {
        let mut candidates = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter_map(|run| {
                let completed_at = run.completed_at?;
                if run.output.is_none() || completed_at + retention > now {
                    return None;
                }
                Some(WorkflowRunOutputRetentionCandidate {
                    run_id: run.id,
                    session_id: run.session_id,
                    expires_at: completed_at + retention,
                })
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            left.expires_at
                .cmp(&right.expires_at)
                .then_with(|| left.run_id.cmp(&right.run_id))
        });
        Ok(candidates)
    }

    pub(super) async fn clear_workflow_run_output(
        &self,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut runs = self.workflow_runs.lock().await;
        let Some(run) = runs.iter_mut().find(|run| run.id == run_id) else {
            return Ok(None);
        };
        run.output = None;
        run.updated_at = Utc::now();
        Ok(Some(run.clone()))
    }

    pub(super) async fn create_credential_binding(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistCredentialBindingRequest,
    ) -> Result<StoredCredentialBinding, SessionStoreError> {
        let now = Utc::now();
        let binding = StoredCredentialBinding {
            id: request.id,
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            provider: request.provider,
            external_ref: request.external_ref,
            namespace: request.namespace,
            allowed_origins: request.allowed_origins,
            injection_mode: request.injection_mode,
            totp: request.totp,
            labels: request.labels,
            created_at: now,
            updated_at: now,
        };
        self.credential_bindings.lock().await.push(binding.clone());
        Ok(binding)
    }

    pub(super) async fn list_credential_bindings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredCredentialBinding>, SessionStoreError> {
        let mut bindings = self
            .credential_bindings
            .lock()
            .await
            .iter()
            .filter(|binding| {
                binding.owner_subject == principal.subject
                    && binding.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        bindings.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(bindings)
    }

    pub(super) async fn get_credential_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredCredentialBinding>, SessionStoreError> {
        Ok(self
            .credential_bindings
            .lock()
            .await
            .iter()
            .find(|binding| {
                binding.id == id
                    && binding.owner_subject == principal.subject
                    && binding.owner_issuer == principal.issuer
            })
            .cloned())
    }
}
