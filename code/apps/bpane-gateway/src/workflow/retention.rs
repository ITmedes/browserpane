use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use tokio::time::sleep;
use tracing::{info, warn};

use super::WorkflowObservability;
use crate::session_control::{SessionStore, SessionStoreError};

#[derive(Clone)]
pub struct WorkflowRetentionManager {
    session_store: SessionStore,
    observability: Arc<WorkflowObservability>,
    interval: Duration,
    log_retention: Option<ChronoDuration>,
    output_retention: Option<ChronoDuration>,
}

impl WorkflowRetentionManager {
    pub fn new(
        session_store: SessionStore,
        observability: Arc<WorkflowObservability>,
        interval: Duration,
        log_retention: Option<ChronoDuration>,
        output_retention: Option<ChronoDuration>,
    ) -> Self {
        Self {
            session_store,
            observability,
            interval,
            log_retention,
            output_retention,
        }
    }

    pub fn start(self: Arc<Self>) {
        tokio::spawn(async move {
            loop {
                sleep(self.interval).await;
                if let Err(error) = self.run_cleanup_pass(Utc::now()).await {
                    warn!("workflow retention cleanup pass failed: {error}");
                }
            }
        });
    }

    pub async fn run_cleanup_pass(&self, now: DateTime<Utc>) -> Result<(), SessionStoreError> {
        let log_candidates = match self.log_retention {
            Some(retention) => {
                self.session_store
                    .list_workflow_run_log_retention_candidates(now, retention)
                    .await?
            }
            None => Vec::new(),
        };
        let output_candidates = match self.output_retention {
            Some(retention) => {
                self.session_store
                    .list_workflow_run_output_retention_candidates(now, retention)
                    .await?
            }
            None => Vec::new(),
        };

        self.observability
            .record_retention_pass(now, log_candidates.len(), output_candidates.len())
            .await;

        for candidate in log_candidates {
            match self
                .session_store
                .delete_workflow_run_logs(candidate.run_id, candidate.automation_task_id)
                .await
            {
                Ok(deleted) => {
                    self.observability.record_retention_deleted_logs(deleted);
                    if deleted > 0 {
                        info!(
                            run_id = %candidate.run_id,
                            session_id = %candidate.session_id,
                            expires_at = %candidate.expires_at,
                            deleted,
                            "deleted retained workflow logs after expiration"
                        );
                    }
                }
                Err(error) => {
                    self.observability.record_retention_failure();
                    warn!(
                        run_id = %candidate.run_id,
                        session_id = %candidate.session_id,
                        expires_at = %candidate.expires_at,
                        "failed to delete retained workflow logs: {error}"
                    );
                }
            }
        }

        for candidate in output_candidates {
            match self
                .session_store
                .clear_workflow_run_output(candidate.run_id)
                .await
            {
                Ok(Some(_)) => {
                    self.observability.record_retention_cleared_output();
                    info!(
                        run_id = %candidate.run_id,
                        session_id = %candidate.session_id,
                        expires_at = %candidate.expires_at,
                        "cleared retained workflow output after expiration"
                    );
                }
                Ok(None) => {}
                Err(error) => {
                    self.observability.record_retention_failure();
                    warn!(
                        run_id = %candidate.run_id,
                        session_id = %candidate.session_id,
                        expires_at = %candidate.expires_at,
                        "failed to clear retained workflow output: {error}"
                    );
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Duration;

    use crate::auth::AuthenticatedPrincipal;
    use crate::automation_task::{
        AutomationTaskLogStream, AutomationTaskSessionSource, PersistAutomationTaskRequest,
    };
    use crate::session_control::{CreateSessionRequest, SessionOwnerMode, SessionRecordingPolicy};
    use crate::workflow::{
        PersistWorkflowDefinitionRequest, PersistWorkflowDefinitionVersionRequest,
        PersistWorkflowRunLogRequest, PersistWorkflowRunRequest, WorkflowRunState,
        WorkflowRunTransitionRequest,
    };

    use super::*;

    fn owner() -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: "owner".to_string(),
            issuer: "issuer".to_string(),
            display_name: Some("Owner".to_string()),
            client_id: None,
        }
    }

    #[tokio::test]
    async fn cleanup_pass_removes_expired_logs_and_outputs() {
        let store = SessionStore::in_memory();
        let principal = owner();
        let session = store
            .create_session(
                &principal,
                CreateSessionRequest {
                    template_id: None,
                    owner_mode: None,
                    viewport: None,
                    idle_timeout_sec: None,
                    labels: HashMap::new(),
                    integration_context: None,
                    extension_ids: Vec::new(),
                    extensions: Vec::new(),
                    recording: SessionRecordingPolicy::default(),
                },
                SessionOwnerMode::Collaborative,
            )
            .await
            .unwrap();
        let task = store
            .create_automation_task(
                &principal,
                PersistAutomationTaskRequest {
                    display_name: Some("workflow task".to_string()),
                    executor: "playwright".to_string(),
                    session_id: session.id,
                    session_source: AutomationTaskSessionSource::ExistingSession,
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let workflow = store
            .create_workflow_definition(
                &principal,
                PersistWorkflowDefinitionRequest {
                    name: "workflow".to_string(),
                    description: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap();
        let version = store
            .create_workflow_definition_version(
                &principal,
                PersistWorkflowDefinitionVersionRequest {
                    workflow_definition_id: workflow.id,
                    version: "v1".to_string(),
                    executor: "playwright".to_string(),
                    entrypoint: "workflows/test.mjs".to_string(),
                    source: None,
                    input_schema: None,
                    output_schema: None,
                    default_session: None,
                    allowed_credential_binding_ids: Vec::new(),
                    allowed_extension_ids: Vec::new(),
                    allowed_file_workspace_ids: Vec::new(),
                },
            )
            .await
            .unwrap();
        let run = store
            .create_workflow_run(
                &principal,
                PersistWorkflowRunRequest {
                    workflow_definition_id: workflow.id,
                    workflow_definition_version_id: version.id,
                    workflow_version: version.version.clone(),
                    session_id: session.id,
                    automation_task_id: task.id,
                    source_system: None,
                    source_reference: None,
                    client_request_id: None,
                    create_request_fingerprint: None,
                    source_snapshot: None,
                    extensions: Vec::new(),
                    credential_bindings: Vec::new(),
                    workspace_inputs: Vec::new(),
                    input: None,
                    labels: HashMap::new(),
                },
            )
            .await
            .unwrap()
            .run;
        store
            .append_workflow_run_log(
                run.id,
                PersistWorkflowRunLogRequest {
                    stream: AutomationTaskLogStream::System,
                    message: "run log".to_string(),
                },
            )
            .await
            .unwrap();
        store
            .append_automation_task_log(
                task.id,
                AutomationTaskLogStream::Stdout,
                "task log".to_string(),
            )
            .await
            .unwrap();
        store
            .transition_workflow_run(
                run.id,
                WorkflowRunTransitionRequest {
                    state: WorkflowRunState::Starting,
                    output: None,
                    error: None,
                    artifact_refs: Vec::new(),
                    message: Some("starting".to_string()),
                    data: None,
                },
            )
            .await
            .unwrap()
            .unwrap();
        store
            .transition_workflow_run(
                run.id,
                WorkflowRunTransitionRequest {
                    state: WorkflowRunState::Running,
                    output: None,
                    error: None,
                    artifact_refs: Vec::new(),
                    message: Some("running".to_string()),
                    data: None,
                },
            )
            .await
            .unwrap()
            .unwrap();
        let succeeded = store
            .transition_workflow_run(
                run.id,
                WorkflowRunTransitionRequest {
                    state: WorkflowRunState::Succeeded,
                    output: Some(serde_json::json!({ "ok": true })),
                    error: None,
                    artifact_refs: Vec::new(),
                    message: Some("done".to_string()),
                    data: None,
                },
            )
            .await
            .unwrap()
            .unwrap();

        let manager = Arc::new(WorkflowRetentionManager::new(
            store.clone(),
            Arc::new(WorkflowObservability::default()),
            Duration::from_secs(60),
            Some(ChronoDuration::seconds(60)),
            Some(ChronoDuration::seconds(60)),
        ));
        manager
            .run_cleanup_pass(succeeded.completed_at.unwrap() + ChronoDuration::seconds(61))
            .await
            .unwrap();

        let reloaded = store
            .get_workflow_run_for_owner(&principal, run.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.output, None);
        let logs = store
            .list_workflow_run_logs_for_owner(&principal, run.id)
            .await
            .unwrap();
        assert!(logs.is_empty());
        let task_logs = store
            .list_automation_task_logs_for_owner(&principal, task.id)
            .await
            .unwrap();
        assert!(task_logs.is_empty());
    }
}
