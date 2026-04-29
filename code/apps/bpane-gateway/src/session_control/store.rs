use super::*;

#[derive(Debug, Clone)]
pub enum SessionStoreError {
    ActiveSessionConflict { max_runtime_sessions: usize },
    Conflict(String),
    NotFound(String),
    InvalidRequest(String),
    Backend(String),
}

impl std::fmt::Display for SessionStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ActiveSessionConflict {
                max_runtime_sessions,
            } => {
                write!(
                    f,
                    "the current gateway runtime only supports {} active runtime-backed session{}",
                    max_runtime_sessions,
                    if *max_runtime_sessions == 1 { "" } else { "s" }
                )
            }
            Self::Conflict(message) => write!(f, "{message}"),
            Self::NotFound(message) => write!(f, "{message}"),
            Self::InvalidRequest(message) => write!(f, "{message}"),
            Self::Backend(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SessionStoreError {}

#[derive(Clone)]
pub struct SessionStore {
    backend: SessionStoreBackend,
}

#[derive(Debug, Clone)]
pub(super) struct SessionStoreConfig {
    pub(super) runtime_binding: String,
    pub(super) max_runtime_candidates: usize,
}

#[derive(Clone)]
enum SessionStoreBackend {
    InMemory(Arc<InMemorySessionStore>),
    Postgres(Arc<PostgresSessionStore>),
}

impl From<SessionManagerProfile> for SessionStoreConfig {
    fn from(runtime_profile: SessionManagerProfile) -> Self {
        Self {
            runtime_binding: runtime_profile.runtime_binding,
            max_runtime_candidates: runtime_profile.max_runtime_sessions,
        }
    }
}

#[cfg(test)]
fn legacy_runtime_profile() -> SessionManagerProfile {
    SessionManagerProfile {
        runtime_binding: "legacy_single_session".to_string(),
        compatibility_mode: "legacy_single_runtime".to_string(),
        max_runtime_sessions: 1,
        supports_legacy_global_routes: true,
        supports_session_extensions: false,
    }
}

impl SessionStore {
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self::in_memory_with_config(legacy_runtime_profile())
    }

    pub fn in_memory_with_config(runtime_profile: SessionManagerProfile) -> Self {
        Self {
            backend: SessionStoreBackend::InMemory(Arc::new(InMemorySessionStore::new(
                SessionStoreConfig::from(runtime_profile),
            ))),
        }
    }

    pub async fn from_database_url_with_config(
        database_url: &str,
        runtime_profile: SessionManagerProfile,
    ) -> Result<Self, SessionStoreError> {
        run_postgres_migrations(database_url).await?;
        let (store, connection) =
            PostgresSessionStore::connect(database_url, SessionStoreConfig::from(runtime_profile))
                .await?;
        tokio::spawn(async move {
            if let Err(error) = connection.await {
                tracing::error!("postgres connection error: {error}");
            }
        });
        Ok(Self {
            backend: SessionStoreBackend::Postgres(Arc::new(store)),
        })
    }

    pub async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        validate_create_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_session(principal, request, owner_mode).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_session(principal, request, owner_mode).await
            }
        }
    }

    pub async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_sessions_for_owner(principal).await,
            SessionStoreBackend::Postgres(store) => store.list_sessions_for_owner(principal).await,
        }
    }

    pub async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_session_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_session_for_owner(principal, id).await
            }
        }
    }

    pub async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.stop_session_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.stop_session_for_owner(principal, id).await
            }
        }
    }

    pub async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_session_for_principal(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_session_for_principal(principal, id).await
            }
        }
    }

    pub async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_session_by_id(id).await,
            SessionStoreBackend::Postgres(store) => store.get_session_by_id(id).await,
        }
    }

    pub async fn prepare_session_for_connect(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.prepare_session_for_connect(id).await,
            SessionStoreBackend::Postgres(store) => store.prepare_session_for_connect(id).await,
        }
    }

    pub async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_runtime_candidate_session().await,
            SessionStoreBackend::Postgres(store) => store.get_runtime_candidate_session().await,
        }
    }

    pub async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        validate_automation_delegate_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .set_automation_delegate_for_owner(principal, id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .set_automation_delegate_for_owner(principal, id, request)
                    .await
            }
        }
    }

    pub async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .clear_automation_delegate_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .clear_automation_delegate_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn create_automation_task(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistAutomationTaskRequest,
    ) -> Result<StoredAutomationTask, SessionStoreError> {
        validate_persist_automation_task_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.create_automation_task(principal, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.create_automation_task(principal, request).await
            }
        }
    }

    pub async fn list_automation_tasks_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_automation_tasks_for_owner(principal).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_automation_tasks_for_owner(principal).await
            }
        }
    }

    pub async fn get_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_automation_task_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_automation_task_for_owner(principal, id).await
            }
        }
    }

    pub async fn get_automation_task_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.get_automation_task_by_id(id).await,
            SessionStoreBackend::Postgres(store) => store.get_automation_task_by_id(id).await,
        }
    }

    pub async fn cancel_automation_task_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.cancel_automation_task_for_owner(principal, id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.cancel_automation_task_for_owner(principal, id).await
            }
        }
    }

    pub async fn list_automation_task_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskEvent>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_automation_task_events_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_automation_task_events_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn list_automation_task_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredAutomationTaskLog>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_automation_task_logs_for_owner(principal, id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_automation_task_logs_for_owner(principal, id)
                    .await
            }
        }
    }

    pub async fn transition_automation_task(
        &self,
        id: Uuid,
        request: AutomationTaskTransitionRequest,
    ) -> Result<Option<StoredAutomationTask>, SessionStoreError> {
        validate_automation_task_transition_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.transition_automation_task(id, request).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.transition_automation_task(id, request).await
            }
        }
    }

    pub async fn append_automation_task_log(
        &self,
        id: Uuid,
        stream: AutomationTaskLogStream,
        message: String,
    ) -> Result<Option<StoredAutomationTaskLog>, SessionStoreError> {
        validate_automation_task_log_message(&message)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.append_automation_task_log(id, stream, message).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.append_automation_task_log(id, stream, message).await
            }
        }
    }

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

    pub async fn mark_session_active(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Active)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Active)
                    .await
            }
        }
    }

    pub async fn mark_session_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Idle)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .mark_session_state(id, SessionLifecycleState::Idle)
                    .await
            }
        }
    }

    pub async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.stop_session_if_idle(id).await,
            SessionStoreBackend::Postgres(store) => store.stop_session_if_idle(id).await,
        }
    }

    pub async fn upsert_runtime_assignment(
        &self,
        assignment: PersistedSessionRuntimeAssignment,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.upsert_runtime_assignment(assignment).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.upsert_runtime_assignment(assignment).await
            }
        }
    }

    pub async fn create_recording_for_session(
        &self,
        session_id: Uuid,
        format: SessionRecordingFormat,
        previous_recording_id: Option<Uuid>,
    ) -> Result<StoredSessionRecording, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .create_recording_for_session(session_id, format, previous_recording_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .create_recording_for_session(session_id, format, previous_recording_id)
                    .await
            }
        }
    }

    pub async fn list_recordings_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_recordings_for_session(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_recordings_for_session(session_id).await
            }
        }
    }

    pub async fn get_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .get_recording_for_session(session_id, recording_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .get_recording_for_session(session_id, recording_id)
                    .await
            }
        }
    }

    pub async fn get_latest_recording_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_latest_recording_for_session(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_latest_recording_for_session(session_id).await
            }
        }
    }

    pub async fn list_recording_artifact_retention_candidates(
        &self,
        now: DateTime<Utc>,
    ) -> Result<Vec<RecordingArtifactRetentionCandidate>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .list_recording_artifact_retention_candidates(now)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .list_recording_artifact_retention_candidates(now)
                    .await
            }
        }
    }

    pub async fn stop_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        termination_reason: SessionRecordingTerminationReason,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .stop_recording_for_session(session_id, recording_id, termination_reason)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .stop_recording_for_session(session_id, recording_id, termination_reason)
                    .await
            }
        }
    }

    pub async fn clear_recording_artifact_path(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .clear_recording_artifact_path(session_id, recording_id)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .clear_recording_artifact_path(session_id, recording_id)
                    .await
            }
        }
    }

    pub async fn complete_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: PersistCompletedSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        validate_persist_completed_recording_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .complete_recording_for_session(session_id, recording_id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .complete_recording_for_session(session_id, recording_id, request)
                    .await
            }
        }
    }

    pub async fn fail_recording_for_session(
        &self,
        session_id: Uuid,
        recording_id: Uuid,
        request: FailSessionRecordingRequest,
    ) -> Result<Option<StoredSessionRecording>, SessionStoreError> {
        validate_fail_recording_request(&request)?;
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store
                    .fail_recording_for_session(session_id, recording_id, request)
                    .await
            }
            SessionStoreBackend::Postgres(store) => {
                store
                    .fail_recording_for_session(session_id, recording_id, request)
                    .await
            }
        }
    }

    pub async fn clear_runtime_assignment(&self, id: Uuid) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.clear_runtime_assignment(id).await,
            SessionStoreBackend::Postgres(store) => store.clear_runtime_assignment(id).await,
        }
    }

    pub async fn upsert_recording_worker_assignment(
        &self,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.upsert_recording_worker_assignment(assignment).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.upsert_recording_worker_assignment(assignment).await
            }
        }
    }

    pub async fn clear_recording_worker_assignment(
        &self,
        session_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.clear_recording_worker_assignment(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.clear_recording_worker_assignment(session_id).await
            }
        }
    }

    pub async fn get_recording_worker_assignment(
        &self,
        session_id: Uuid,
    ) -> Result<Option<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.get_recording_worker_assignment(session_id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.get_recording_worker_assignment(session_id).await
            }
        }
    }

    pub async fn list_recording_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => store.list_recording_worker_assignments().await,
            SessionStoreBackend::Postgres(store) => store.list_recording_worker_assignments().await,
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

    pub async fn list_runtime_assignments(
        &self,
        runtime_binding: &str,
    ) -> Result<Vec<PersistedSessionRuntimeAssignment>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.list_runtime_assignments(runtime_binding).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.list_runtime_assignments(runtime_binding).await
            }
        }
    }

    pub async fn mark_session_ready_after_runtime_loss(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        match &self.backend {
            SessionStoreBackend::InMemory(store) => {
                store.mark_session_ready_after_runtime_loss(id).await
            }
            SessionStoreBackend::Postgres(store) => {
                store.mark_session_ready_after_runtime_loss(id).await
            }
        }
    }
}
