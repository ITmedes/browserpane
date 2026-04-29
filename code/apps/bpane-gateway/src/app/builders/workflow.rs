use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use chrono::{Duration as ChronoDuration, Utc};

use crate::auth::AuthValidator;
use crate::config::Config;
use crate::session_access::SessionAutomationAccessTokenManager;
use crate::session_control::SessionStore;
use crate::session_manager::SessionManager;
use crate::session_registry::SessionRegistry;
use crate::workflow::{WorkflowObservability, WorkflowRetentionManager, WorkflowSourceResolver};
use crate::workflow_event_delivery::{WorkflowEventDeliveryConfig, WorkflowEventDeliveryManager};
use crate::workflow_lifecycle::{WorkflowLifecycleManager, WorkflowWorkerConfig};

use super::WorkflowServices;

impl WorkflowServices {
    pub(in crate::app) async fn build(
        config: &Config,
        auth_validator: Arc<AuthValidator>,
        automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
        session_store: SessionStore,
        session_manager: Arc<SessionManager>,
        registry: Arc<SessionRegistry>,
    ) -> anyhow::Result<Self> {
        let source_resolver = Arc::new(WorkflowSourceResolver::new(
            config.workflow.workflow_git_bin.clone(),
        ));
        let lifecycle = Arc::new(WorkflowLifecycleManager::new(
            build_workflow_worker_config(config),
            auth_validator,
            automation_access_token_manager,
            session_store.clone(),
            session_manager,
            registry,
        )?);
        lifecycle.reconcile_persisted_state().await?;

        let observability = Arc::new(WorkflowObservability::default());
        let event_delivery = Arc::new(WorkflowEventDeliveryManager::new(
            session_store.clone(),
            observability.clone(),
            WorkflowEventDeliveryConfig {
                poll_interval: Duration::from_millis(
                    config.workflow.workflow_event_delivery_poll_interval_ms,
                ),
                request_timeout: Duration::from_secs(
                    config.workflow.workflow_event_delivery_timeout_secs,
                ),
                max_attempts: config.workflow.workflow_event_delivery_max_attempts,
                batch_size: config.workflow.workflow_event_delivery_batch_size,
                base_backoff: Duration::from_secs(
                    config.workflow.workflow_event_delivery_base_backoff_secs,
                ),
            },
        )?);
        event_delivery.reconcile_persisted_state().await?;
        event_delivery.start();

        let log_retention = workflow_retention_window(
            "workflow-log-retention-secs",
            config.workflow.workflow_log_retention_secs,
        )?;
        let output_retention = workflow_retention_window(
            "workflow-output-retention-secs",
            config.workflow.workflow_output_retention_secs,
        )?;
        if config.workflow.workflow_retention_cleanup_interval_secs > 0
            && (log_retention.is_some() || output_retention.is_some())
        {
            let retention = Arc::new(WorkflowRetentionManager::new(
                session_store,
                observability.clone(),
                Duration::from_secs(config.workflow.workflow_retention_cleanup_interval_secs),
                log_retention,
                output_retention,
            ));
            retention.run_cleanup_pass(Utc::now()).await?;
            retention.start();
        }

        Ok(Self {
            source_resolver,
            lifecycle,
            observability,
            log_retention,
            output_retention,
        })
    }
}

fn build_workflow_worker_config(config: &Config) -> Option<WorkflowWorkerConfig> {
    config
        .workflow
        .workflow_worker_image
        .clone()
        .map(|image| WorkflowWorkerConfig {
            docker_bin: config.workflow.workflow_worker_docker_bin.clone(),
            image,
            max_active_workers: config.workflow.workflow_worker_max_active,
            network: config.workflow.workflow_worker_network.clone(),
            container_name_prefix: config
                .workflow
                .workflow_worker_container_name_prefix
                .clone(),
            gateway_api_url: config.workflow.workflow_worker_api_url.clone(),
            work_root: config.workflow.workflow_worker_work_root.clone(),
            bearer_token: config.workflow.workflow_worker_bearer_token.clone(),
            oidc_token_url: config.workflow.workflow_worker_oidc_token_url.clone(),
            oidc_client_id: config.workflow.workflow_worker_oidc_client_id.clone(),
            oidc_client_secret: config.workflow.workflow_worker_oidc_client_secret.clone(),
            oidc_scopes: config.workflow.workflow_worker_oidc_scopes.clone(),
        })
}

pub(in crate::app) fn workflow_retention_window(
    flag_name: &str,
    retention_secs: u64,
) -> anyhow::Result<Option<ChronoDuration>> {
    if retention_secs == 0 {
        return Ok(None);
    }

    Ok(Some(ChronoDuration::seconds(
        i64::try_from(retention_secs).map_err(|error| {
            anyhow!("--{flag_name} is out of range for chrono duration: {error}")
        })?,
    )))
}
