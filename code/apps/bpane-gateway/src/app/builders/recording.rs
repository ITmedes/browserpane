use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use chrono::Utc;

use crate::auth::AuthValidator;
use crate::config::Config;
use crate::recording::{RecordingArtifactStore, RecordingObservability, RecordingRetentionManager};
use crate::recording_lifecycle::{RecordingLifecycleManager, RecordingWorkerConfig};
use crate::session_access::{
    RecordingWorkerAccessTokenManager, SessionAutomationAccessTokenManager,
    SessionConnectTicketManager,
};
use crate::session_control::SessionStore;

use super::RecordingServices;

impl RecordingServices {
    pub(in crate::app) async fn build(
        config: &Config,
        auth_validator: Arc<AuthValidator>,
        connect_ticket_manager: Arc<SessionConnectTicketManager>,
        automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
        recording_worker_access_token_manager: Arc<RecordingWorkerAccessTokenManager>,
        session_store: SessionStore,
    ) -> anyhow::Result<Self> {
        let lifecycle = Arc::new(RecordingLifecycleManager::new(
            build_recording_worker_config(config)?,
            auth_validator,
            connect_ticket_manager,
            automation_access_token_manager,
            recording_worker_access_token_manager,
            session_store.clone(),
        )?);
        lifecycle.reconcile_persisted_state().await?;

        let observability = Arc::new(RecordingObservability::default());
        let artifact_store = Arc::new(RecordingArtifactStore::local_fs(
            config.storage.recording_artifact_local_root.clone(),
            config.recording.recording_worker_output_root.clone(),
        ));

        if config.recording.recording_artifact_cleanup_interval_secs > 0 {
            let retention = Arc::new(RecordingRetentionManager::new(
                session_store,
                artifact_store.clone(),
                observability.clone(),
                Duration::from_secs(config.recording.recording_artifact_cleanup_interval_secs),
            ));
            retention.run_cleanup_pass(Utc::now()).await?;
            retention.start();
        }

        Ok(Self {
            lifecycle,
            artifact_store,
            observability,
        })
    }
}

pub(in crate::app) fn build_recording_worker_config(
    config: &Config,
) -> anyhow::Result<Option<RecordingWorkerConfig>> {
    let Some(bin) = config.recording.recording_worker_bin.clone() else {
        return Ok(None);
    };
    let chrome_executable = config
        .recording
        .recording_worker_chrome
        .clone()
        .ok_or_else(|| {
            anyhow!("--recording-worker-chrome is required when --recording-worker-bin is set")
        })?;
    Ok(Some(RecordingWorkerConfig {
        bin,
        args: config.recording.recording_worker_args.clone(),
        chrome_executable,
        gateway_api_url: config.recording.recording_worker_api_url.clone(),
        page_url: config.recording.recording_worker_page_url.clone(),
        output_root: config.recording.recording_worker_output_root.clone(),
        cert_spki: recording_worker_cert_spki(config)?,
        headless: config.recording.recording_worker_headless,
        connect_timeout: Duration::from_secs(
            config.recording.recording_worker_connect_timeout_secs,
        ),
        poll_interval: Duration::from_millis(config.recording.recording_worker_poll_interval_ms),
        finalize_timeout: Duration::from_secs(
            config.recording.recording_worker_finalize_timeout_secs,
        ),
        request_timeout: Duration::from_millis(
            config.recording.recording_worker_request_timeout_ms,
        ),
        output_limit_bytes: config.recording.recording_worker_output_limit_bytes,
        bearer_token: config.recording.recording_worker_bearer_token.clone(),
        oidc_token_url: config.recording.recording_worker_oidc_token_url.clone(),
        oidc_client_id: config.recording.recording_worker_oidc_client_id.clone(),
        oidc_client_secret: config.recording.recording_worker_oidc_client_secret.clone(),
        oidc_scopes: config.recording.recording_worker_oidc_scopes.clone(),
    }))
}

fn recording_worker_cert_spki(config: &Config) -> anyhow::Result<Option<String>> {
    if let Some(value) = config
        .recording
        .recording_worker_cert_spki
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return Ok(Some(value.to_string()));
    }

    let Some(path) = &config.recording.recording_worker_cert_spki_file else {
        return Ok(None);
    };
    let value = std::fs::read_to_string(path).with_context(|| {
        format!(
            "failed to read --recording-worker-cert-spki-file {}",
            path.display()
        )
    })?;
    let value = value.trim();
    if value.is_empty() {
        return Err(anyhow!(
            "--recording-worker-cert-spki-file {} is empty",
            path.display()
        ));
    }
    Ok(Some(value.to_string()))
}
