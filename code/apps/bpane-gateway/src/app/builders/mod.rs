use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{anyhow, bail};
use chrono::Duration as ChronoDuration;
use wtransport::Identity;

use crate::auth::AuthValidator;
use crate::config::Config;
use crate::credentials::{CredentialProvider, VaultKvV2CredentialProvider};
use crate::recording::{RecordingArtifactStore, RecordingObservability};
use crate::recording_lifecycle::RecordingLifecycleManager;
use crate::session_access::{
    AdminEventAccessTokenManager, RecordingWorkerAccessTokenManager,
    SessionAutomationAccessTokenManager, SessionConnectTicketManager,
};
use crate::session_control::{SessionOwnerMode, SessionStore};
use crate::session_manager::SessionManager;
use crate::session_registry::SessionRegistry;
use crate::workflow::{WorkflowObservability, WorkflowSourceResolver};
use crate::workflow_event_delivery::WorkflowEventDestinationPolicy;
use crate::workflow_lifecycle::WorkflowLifecycleManager;

mod auth;
mod browser_contexts;
mod recording;
mod runtime;
mod secret_file;
mod session_files;
mod workflow;

#[cfg(test)]
pub(in crate::app) use self::auth::load_or_generate_shared_secret;
pub(in crate::app) use self::browser_contexts::start_browser_context_retention;
#[cfg(test)]
pub(in crate::app) use self::recording::build_recording_worker_config;
#[cfg(test)]
pub(in crate::app) use self::runtime::{build_session_manager_config, resolve_database_url};
pub(in crate::app) use self::secret_file::{
    load_secret_file, resolve_optional_secret, SecretFilePermissions,
};
#[cfg(test)]
pub(in crate::app) use self::session_files::session_file_retention_window;
pub(in crate::app) use self::session_files::start_session_file_retention;
#[cfg(test)]
pub(in crate::app) use self::workflow::workflow_retention_window;

pub(super) struct AuthServices {
    pub(super) auth_validator: Arc<AuthValidator>,
    pub(super) admin_event_access_token_manager: Arc<AdminEventAccessTokenManager>,
    pub(super) connect_ticket_manager: Arc<SessionConnectTicketManager>,
    pub(super) automation_access_token_manager: Arc<SessionAutomationAccessTokenManager>,
    pub(super) recording_worker_access_token_manager: Arc<RecordingWorkerAccessTokenManager>,
}

pub(super) struct RuntimeServices {
    pub(super) bind_addr: SocketAddr,
    pub(super) api_bind_addr: SocketAddr,
    pub(super) identity: Identity,
    pub(super) registry: Arc<SessionRegistry>,
    pub(super) session_manager: Arc<SessionManager>,
    pub(super) session_store: SessionStore,
}

pub(super) struct RecordingServices {
    pub(super) lifecycle: Arc<RecordingLifecycleManager>,
    pub(super) artifact_store: Arc<RecordingArtifactStore>,
    pub(super) observability: Arc<RecordingObservability>,
}

pub(super) struct WorkflowServices {
    pub(super) source_resolver: Arc<WorkflowSourceResolver>,
    pub(super) lifecycle: Arc<WorkflowLifecycleManager>,
    pub(super) observability: Arc<WorkflowObservability>,
    pub(super) event_destination_policy: Arc<WorkflowEventDestinationPolicy>,
    pub(super) log_retention: Option<ChronoDuration>,
    pub(super) output_retention: Option<ChronoDuration>,
}

pub(super) fn build_credential_provider(
    config: &Config,
) -> anyhow::Result<Option<Arc<CredentialProvider>>> {
    let token = resolve_optional_secret(
        config.storage.credential_vault_token.as_deref(),
        config.storage.credential_vault_token_file.as_deref(),
        "--credential-vault-token",
        "--credential-vault-token-file",
        SecretFilePermissions::OwnerOnly,
    )?;
    match (config.storage.credential_vault_addr.clone(), token) {
        (Some(addr), Some(token)) => Ok(Some(Arc::new(CredentialProvider::new(Arc::new(
            VaultKvV2CredentialProvider::new(
                addr,
                token,
                config.storage.credential_vault_mount_path.clone(),
                Some(config.storage.credential_vault_prefix.clone()),
            )?,
        ))))),
        (None, None) => Ok(None),
        _ => bail!("--credential-vault-addr and a credential Vault token must be set together"),
    }
}

pub(super) fn default_owner_mode(config: &Config) -> SessionOwnerMode {
    if config.gateway.exclusive_browser_owner {
        SessionOwnerMode::ExclusiveBrowserOwner
    } else {
        SessionOwnerMode::Collaborative
    }
}

fn required_string(
    value: &Option<String>,
    flag_name: &str,
    runtime_backend: &str,
) -> anyhow::Result<String> {
    value
        .clone()
        .ok_or_else(|| anyhow!("{flag_name} is required for --runtime-backend {runtime_backend}"))
}
