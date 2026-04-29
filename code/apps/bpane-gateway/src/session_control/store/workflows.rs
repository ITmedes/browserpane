use super::*;

impl SessionStore {
    pub async fn create_workflow_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionRequest,
    ) -> Result<StoredWorkflowDefinition, SessionStoreError> {
        validate_workflow_definition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_workflow_definition(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_workflow_definition(principal, request).await
            }
        }
    }

    pub async fn list_workflow_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_workflow_definitions_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_workflow_definitions_for_owner(principal).await
            }
        }
    }

    pub async fn get_workflow_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinition>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_definition_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_definition_for_owner(principal, id).await
            }
        }
    }

    pub async fn create_workflow_definition_version(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowDefinitionVersionRequest,
    ) -> Result<StoredWorkflowDefinitionVersion, SessionStoreError> {
        validate_workflow_definition_version_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_workflow_definition_version(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_workflow_definition_version(principal, request)
                    .await
            }
        }
    }

    pub async fn get_workflow_definition_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        workflow_definition_id: Uuid,
        version: &str,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_workflow_definition_version_for_owner(
                        principal,
                        workflow_definition_id,
                        version,
                    )
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_workflow_definition_version_for_owner(
                        principal,
                        workflow_definition_id,
                        version,
                    )
                    .await
            }
        }
    }

    pub async fn get_workflow_definition_version_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowDefinitionVersion>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_definition_version_by_id(id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_definition_version_by_id(id).await
            }
        }
    }

    pub async fn create_workflow_run(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowRunRequest,
    ) -> Result<CreateWorkflowRunResult, SessionStoreError> {
        validate_workflow_run_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_workflow_run(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_workflow_run(principal, request).await
            }
        }
    }

    pub async fn get_workflow_run_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_run_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_run_for_owner(principal, id).await
            }
        }
    }

    pub async fn get_workflow_run_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_workflow_run_by_id(id).await,
            SessionStoreBackend::Postgres(store) => store.get_workflow_run_by_id(id).await,
        }
    }

    pub async fn list_dispatchable_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_dispatchable_workflow_runs().await,
            SessionStoreBackend::Postgres(store) => store.list_dispatchable_workflow_runs().await,
        }
    }

    pub async fn find_workflow_run_by_client_request_id_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        client_request_id: &str,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .find_workflow_run_by_client_request_id_for_owner(principal, client_request_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .find_workflow_run_by_client_request_id_for_owner(principal, client_request_id)
                    .await
            }
        }
    }

    pub async fn list_workflow_run_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_run_events_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_run_events_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn list_workflow_run_events(
        &self,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_workflow_run_events(id).await,
            SessionStoreBackend::Postgres(store) => store.list_workflow_run_events(id).await,
        }
    }

    pub async fn list_workflow_run_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunLog>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_workflow_run_logs_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_workflow_run_logs_for_owner(principal, id).await
            }
        }
    }

    pub async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        validate_workflow_event_subscription_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_workflow_event_subscription(principal, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_workflow_event_subscription(principal, request)
                    .await
            }
        }
    }

    pub async fn list_workflow_event_subscriptions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowEventSubscription>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_event_subscriptions_for_owner(principal)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_event_subscriptions_for_owner(principal)
                    .await
            }
        }
    }

    pub async fn get_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_workflow_event_subscription_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_workflow_event_subscription_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn delete_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .delete_workflow_event_subscription_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .delete_workflow_event_subscription_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn list_workflow_event_deliveries_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_event_deliveries_for_owner(principal, subscription_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_event_deliveries_for_owner(principal, subscription_id)
                    .await
            }
        }
    }

    pub async fn list_workflow_event_delivery_attempts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDeliveryAttempt>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_event_delivery_attempts_for_owner(principal, subscription_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_event_delivery_attempts_for_owner(principal, subscription_id)
                    .await
            }
        }
    }

    pub async fn requeue_inflight_workflow_event_deliveries(
        &self,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.requeue_inflight_workflow_event_deliveries().await
            }
            SessionStoreBackend::Postgres(store) => {
                store.requeue_inflight_workflow_event_deliveries().await
            }
        }
    }

    pub async fn claim_due_workflow_event_deliveries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.claim_due_workflow_event_deliveries(limit, now).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.claim_due_workflow_event_deliveries(limit, now).await
            }
        }
    }

    pub async fn record_workflow_event_delivery_attempt(
        &self,
        delivery_id: Uuid,
        request: RecordWorkflowEventDeliveryAttemptRequest,
    ) -> Result<Option<StoredWorkflowEventDelivery>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .record_workflow_event_delivery_attempt(delivery_id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .record_workflow_event_delivery_attempt(delivery_id, request)
                    .await
            }
        }
    }

    pub async fn append_workflow_run_event_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        validate_workflow_run_event_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .append_workflow_run_event_for_owner(principal, id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .append_workflow_run_event_for_owner(principal, id, request)
                    .await
            }
        }
    }

    pub async fn append_workflow_run_event(
        &self,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        validate_workflow_run_event_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.append_workflow_run_event(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.append_workflow_run_event(id, request).await
            }
        }
    }

    pub async fn transition_workflow_run(
        &self,
        id: Uuid,
        request: WorkflowRunTransitionRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        validate_workflow_run_transition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.transition_workflow_run(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.transition_workflow_run(id, request).await
            }
        }
    }

    pub async fn reconcile_workflow_run_from_task(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.reconcile_workflow_run_from_task(id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.reconcile_workflow_run_from_task(id).await
            }
        }
    }

    pub async fn list_awaiting_input_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_awaiting_input_workflow_runs().await,
            SessionStoreBackend::Postgres(store) => store.list_awaiting_input_workflow_runs().await,
        }
    }

    pub async fn append_workflow_run_log(
        &self,
        id: Uuid,
        request: PersistWorkflowRunLogRequest,
    ) -> Result<Option<StoredWorkflowRunLog>, SessionStoreError> {
        validate_workflow_run_log_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.append_workflow_run_log(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.append_workflow_run_log(id, request).await
            }
        }
    }

    pub async fn append_workflow_run_produced_file(
        &self,
        id: Uuid,
        request: PersistWorkflowRunProducedFileRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        validate_workflow_run_produced_file_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.append_workflow_run_produced_file(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.append_workflow_run_produced_file(id, request).await
            }
        }
    }

    pub async fn list_workflow_run_log_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunLogRetentionCandidate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_run_log_retention_candidates(now, retention)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_run_log_retention_candidates(now, retention)
                    .await
            }
        }
    }

    pub async fn delete_workflow_run_logs(
        &self,
        run_id: Uuid,
        automation_task_id: Uuid,
    ) -> Result<usize, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .delete_workflow_run_logs(run_id, automation_task_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .delete_workflow_run_logs(run_id, automation_task_id)
                    .await
            }
        }
    }

    pub async fn list_workflow_run_output_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunOutputRetentionCandidate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_workflow_run_output_retention_candidates(now, retention)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_workflow_run_output_retention_candidates(now, retention)
                    .await
            }
        }
    }

    pub async fn clear_workflow_run_output(
        &self,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.clear_workflow_run_output(run_id).await,
            SessionStoreBackend::Postgres(store) => store.clear_workflow_run_output(run_id).await,
        }
    }
    pub async fn upsert_workflow_run_worker_assignment(
        &self,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .upsert_workflow_run_worker_assignment(assignment)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .upsert_workflow_run_worker_assignment(assignment)
                    .await
            }
        }
    }

    pub async fn clear_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.clear_workflow_run_worker_assignment(run_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.clear_workflow_run_worker_assignment(run_id).await
            }
        }
    }

    pub async fn get_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<Option<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_workflow_run_worker_assignment(run_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_workflow_run_worker_assignment(run_id).await
            }
        }
    }

    pub async fn list_workflow_run_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_workflow_run_worker_assignments().await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_workflow_run_worker_assignments().await
            }
        }
    }
}
