use std::fmt;
use std::path::Path;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::process::Command;
use tokio::time::{sleep, Instant};
use tracing::info;
use uuid::Uuid;

use super::*;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(in crate::runtime_manager) struct DockerNetworkIdentityLaunchOptions {
    pub(in crate::runtime_manager) env: Vec<(String, String)>,
    pub(in crate::runtime_manager) labels: Vec<(String, String)>,
    pub(in crate::runtime_manager) egress_observer: Option<DockerEgressObserverLaunchSummary>,
    pub(in crate::runtime_manager) egress_custom_ca_ref: Option<String>,
    pub(in crate::runtime_manager) egress_custom_ca_path: Option<String>,
    pub(in crate::runtime_manager) egress_proxy_auth: Option<DockerEgressProxyAuthMaterial>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runtime_manager) struct DockerEgressObserverLaunchSummary {
    pub(in crate::runtime_manager) profile_id: Uuid,
    pub(in crate::runtime_manager) profile_name: String,
    pub(in crate::runtime_manager) observation_mode: EgressTrafficObservationMode,
    pub(in crate::runtime_manager) proxy_configured: bool,
    pub(in crate::runtime_manager) proxy_auth_configured: bool,
    pub(in crate::runtime_manager) bypass_rule_count: usize,
    pub(in crate::runtime_manager) custom_ca_configured: bool,
    pub(in crate::runtime_manager) tls_interception_enabled: bool,
    pub(in crate::runtime_manager) sensitive_log_sink_configured: bool,
}

#[derive(Clone, PartialEq, Eq)]
pub(in crate::runtime_manager) struct DockerEgressProxyAuthMaterial {
    pub(in crate::runtime_manager) binding_id: Uuid,
    pub(in crate::runtime_manager) target_path: String,
    pub(in crate::runtime_manager) payload: Vec<u8>,
}

impl fmt::Debug for DockerEgressProxyAuthMaterial {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DockerEgressProxyAuthMaterial")
            .field("binding_id", &self.binding_id)
            .field("target_path", &self.target_path)
            .field("payload", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Deserialize, Serialize)]
struct EgressProxyAuthPayload {
    username: String,
    password: String,
}

impl DockerRuntimeManager {
    #[cfg(test)]
    pub(in crate::runtime_manager) fn docker_run_args(
        &self,
        lease: &RuntimeLease,
        extension_dirs: &[String],
    ) -> Result<Vec<String>, RuntimeManagerError> {
        self.docker_run_args_with_launch_options(
            lease,
            extension_dirs,
            &DockerNetworkIdentityLaunchOptions::default(),
        )
    }

    pub(in crate::runtime_manager) fn docker_run_args_with_launch_options(
        &self,
        lease: &RuntimeLease,
        extension_dirs: &[String],
        launch_options: &DockerNetworkIdentityLaunchOptions,
    ) -> Result<Vec<String>, RuntimeManagerError> {
        let container_name = lease.container_name.as_deref().ok_or_else(|| {
            RuntimeManagerError::StartupFailed(
                "docker lease did not include a container name".to_string(),
            )
        })?;
        let session_data_root = self.session_data_root().to_string();
        let socket_volume_mount_root = self.socket_volume_mount_root();
        let mut args = vec![
            "run".to_string(),
            "-d".to_string(),
            "--rm".to_string(),
            "--name".to_string(),
            container_name.to_string(),
            "--network".to_string(),
            self.config.network.clone(),
            "--network-alias".to_string(),
            container_name.to_string(),
            "-v".to_string(),
            format!("{}:{socket_volume_mount_root}", self.config.socket_volume),
            "-v".to_string(),
            format!(
                "{}:{}",
                self.session_data_volume_for_session(lease.session_id),
                session_data_root
            ),
        ];
        if let Some(profile_volume) = self.profile_volume_for_lease(lease) {
            args.push("-v".to_string());
            args.push(format!(
                "{}:{}",
                profile_volume,
                self.profile_dir_for_session()
            ));
        }
        args.extend([
            "--shm-size".to_string(),
            self.config.shm_size.clone(),
            "--label".to_string(),
            format!("browserpane.session_id={}", lease.session_id),
            "-e".to_string(),
            format!("BPANE_SESSION_ID={}", lease.session_id),
            "-e".to_string(),
            format!("BPANE_SOCKET_PATH={}", lease.agent_socket_path),
            "-e".to_string(),
            format!("BPANE_SESSION_DATA_DIR={session_data_root}"),
            "-e".to_string(),
            format!("BPANE_PROFILE_DIR={}", self.profile_dir_for_session()),
            "-e".to_string(),
            format!("BPANE_UPLOAD_DIR={}", self.upload_dir_for_session()),
            "-e".to_string(),
            format!("BPANE_DOWNLOAD_DIR={}", self.download_dir_for_session()),
            "-e".to_string(),
            format!(
                "BPANE_SESSION_FILE_MOUNTS_DIR={}",
                self.session_file_mounts_root()
            ),
            "-e".to_string(),
            format!(
                "BPANE_SESSION_FILE_BINDINGS_MANIFEST={}",
                self.session_file_manifest_path()
            ),
        ]);
        if !extension_dirs.is_empty() {
            args.push("-e".to_string());
            args.push(format!("BPANE_EXTENSION_DIRS={}", extension_dirs.join(",")));
        }
        for (key, value) in &launch_options.env {
            args.push("-e".to_string());
            args.push(format!("{key}={value}"));
        }
        for (key, value) in &launch_options.labels {
            args.push("--label".to_string());
            args.push(format!("{key}={value}"));
        }

        if self.config.seccomp_unconfined {
            args.push("--security-opt".to_string());
            args.push("seccomp=unconfined".to_string());
        }
        if let Some(env_file) = &self.config.env_file {
            args.push("--env-file".to_string());
            args.push(env_file.display().to_string());
        }

        args.push(self.config.image.clone());
        Ok(args)
    }

    pub(in crate::runtime_manager) fn network_identity_launch_options(
        session: &StoredSession,
        egress_profile: Option<&StoredEgressProfile>,
        trusted_ca_bundle_path: &str,
        proxy_auth_config_path: &str,
    ) -> DockerNetworkIdentityLaunchOptions {
        let mut env = Vec::new();
        let mut labels = Vec::new();
        let mut egress_observer = None;
        let mut egress_custom_ca_ref = None;
        let mut egress_custom_ca_path = None;
        let identity = &session.network_identity;

        if let Some(locale) = identity.locale.as_deref().filter(|value| !value.is_empty()) {
            let posix_locale = posix_locale(locale);
            push_env(&mut env, "LANG", posix_locale.clone());
            push_env(&mut env, "LC_ALL", posix_locale);
            push_env(&mut env, "BPANE_CHROMIUM_LANG", locale.to_string());
            if identity.languages.is_empty() {
                push_env(&mut env, "BPANE_CHROMIUM_ACCEPT_LANG", locale.to_string());
            }
        }
        if !identity.languages.is_empty() {
            push_env(&mut env, "LANGUAGE", identity.languages.join(":"));
            push_env(
                &mut env,
                "BPANE_CHROMIUM_ACCEPT_LANG",
                identity.languages.join(","),
            );
        }
        if let Some(timezone) = identity
            .timezone
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            push_env(&mut env, "TZ", timezone.to_string());
        }
        if let Some(geolocation) = &identity.geolocation {
            push_env(
                &mut env,
                "BPANE_SESSION_GEOLOCATION",
                serde_json::json!({
                    "latitude": geolocation.latitude,
                    "longitude": geolocation.longitude,
                    "accuracy_meters": geolocation.accuracy_meters,
                })
                .to_string(),
            );
        }
        if let Some(user_agent) = identity
            .user_agent
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            push_env(
                &mut env,
                "BPANE_CHROMIUM_USER_AGENT",
                user_agent.to_string(),
            );
        }
        if let Some(browser_identity) = identity
            .browser_identity
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            push_env(
                &mut env,
                "BPANE_BROWSER_IDENTITY",
                browser_identity.to_string(),
            );
        }
        if let Some(profile) = egress_profile {
            let effective = profile.effective_status();
            push_env(&mut env, "BPANE_EGRESS_PROFILE_ID", profile.id.to_string());
            push_env(&mut env, "BPANE_EGRESS_PROFILE_NAME", profile.name.clone());
            push_env(
                &mut env,
                "BPANE_EGRESS_OBSERVATION_MODE",
                profile.traffic_observation.mode.as_str().to_string(),
            );
            push_label(
                &mut labels,
                "browserpane.egress_profile_id",
                profile.id.to_string(),
            );
            push_label(
                &mut labels,
                "browserpane.egress_observation_mode",
                profile.traffic_observation.mode.as_str().to_string(),
            );
            push_label(
                &mut labels,
                "browserpane.egress_proxy_configured",
                profile.proxy.is_some().to_string(),
            );
            push_label(
                &mut labels,
                "browserpane.egress_proxy_auth_configured",
                effective.proxy_auth_configured.to_string(),
            );
            push_label(
                &mut labels,
                "browserpane.egress_bypass_rule_count",
                profile.bypass_rules.len().to_string(),
            );
            push_label(
                &mut labels,
                "browserpane.egress_custom_ca_configured",
                profile.custom_ca.is_some().to_string(),
            );
            push_label(
                &mut labels,
                "browserpane.egress_tls_interception_enabled",
                effective.tls_interception_enabled.to_string(),
            );
            push_label(
                &mut labels,
                "browserpane.egress_sensitive_log_sink_configured",
                effective.sensitive_log_sink_configured.to_string(),
            );
            egress_observer = Some(DockerEgressObserverLaunchSummary {
                profile_id: profile.id,
                profile_name: profile.name.clone(),
                observation_mode: profile.traffic_observation.mode,
                proxy_configured: profile.proxy.is_some(),
                proxy_auth_configured: effective.proxy_auth_configured,
                bypass_rule_count: profile.bypass_rules.len(),
                custom_ca_configured: profile.custom_ca.is_some(),
                tls_interception_enabled: effective.tls_interception_enabled,
                sensitive_log_sink_configured: effective.sensitive_log_sink_configured,
            });
            if let Some(proxy) = &profile.proxy {
                push_env(&mut env, "BPANE_CHROMIUM_PROXY_SERVER", proxy.url.clone());
                if proxy.credential_binding_id.is_some() {
                    push_env(
                        &mut env,
                        "BPANE_CHROMIUM_PROXY_AUTH_FILE",
                        proxy_auth_config_path.to_string(),
                    );
                    push_env(&mut env, "BPANE_URL", "about:blank".to_string());
                }
            }
            if !profile.bypass_rules.is_empty() {
                push_env(
                    &mut env,
                    "BPANE_CHROMIUM_PROXY_BYPASS_LIST",
                    profile.bypass_rules.join(";"),
                );
            }
            if let Some(custom_ca) = &profile.custom_ca {
                push_env(
                    &mut env,
                    "BPANE_CUSTOM_CA_REF",
                    custom_ca.certificate_ref.clone(),
                );
                if profile.traffic_observation.mode == EgressTrafficObservationMode::TlsIntercept {
                    push_env(
                        &mut env,
                        "BPANE_CHROMIUM_TRUSTED_CA_BUNDLE",
                        trusted_ca_bundle_path.to_string(),
                    );
                    push_env(
                        &mut env,
                        "BPANE_CHROMIUM_TRUSTED_CA_NAME",
                        custom_ca
                            .display_name
                            .clone()
                            .unwrap_or_else(|| "BrowserPane Egress Interception CA".to_string()),
                    );
                    egress_custom_ca_ref = Some(custom_ca.certificate_ref.clone());
                    egress_custom_ca_path = Some(trusted_ca_bundle_path.to_string());
                }
            }
        }

        DockerNetworkIdentityLaunchOptions {
            env,
            labels,
            egress_observer,
            egress_custom_ca_ref,
            egress_custom_ca_path,
            egress_proxy_auth: None,
        }
    }

    pub(in crate::runtime_manager) async fn session_network_identity_launch_options(
        &self,
        session_id: Uuid,
    ) -> Result<DockerNetworkIdentityLaunchOptions, RuntimeManagerError> {
        let Some(store) = self.session_store().await else {
            return Ok(DockerNetworkIdentityLaunchOptions::default());
        };
        let session = store
            .get_session_by_id(session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?
            .ok_or_else(|| {
                RuntimeManagerError::PersistenceFailed(format!(
                    "session {session_id} not found while resolving network identity launch options"
                ))
            })?;
        let principal = AuthenticatedPrincipal {
            subject: session.owner.subject.clone(),
            issuer: session.owner.issuer.clone(),
            display_name: session.owner.display_name.clone(),
            client_id: None,
            safe_claims: Default::default(),
        };
        let egress_profile = if let Some(profile_id) = session.network_identity.egress_profile_id {
            let profile = store
                .get_egress_profile_for_owner(&principal, profile_id)
                .await
                .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?
                .ok_or_else(|| {
                    RuntimeManagerError::PersistenceFailed(format!(
                        "egress profile {profile_id} not found while starting session {session_id}"
                    ))
                })?;
            if profile.state == EgressProfileState::Disabled {
                return Err(RuntimeManagerError::StartupFailed(format!(
                    "egress profile {profile_id} is disabled"
                )));
            }
            Some(profile)
        } else {
            None
        };

        let mut launch_options = Self::network_identity_launch_options(
            &session,
            egress_profile.as_ref(),
            &self.egress_custom_ca_bundle_path_for_session(),
            &self.egress_proxy_auth_config_path_for_session(),
        );
        if let Some(profile) = egress_profile.as_ref() {
            launch_options.egress_proxy_auth = self
                .resolve_egress_proxy_auth_material(&principal, profile)
                .await?;
        }
        Ok(launch_options)
    }

    pub(in crate::runtime_manager) fn egress_custom_ca_bundle_path_for_session(&self) -> String {
        format!("{}/egress/custom-ca.pem", self.session_data_root())
    }

    pub(in crate::runtime_manager) fn egress_proxy_auth_config_path_for_session(&self) -> String {
        format!("{}/egress/proxy-auth.json", self.session_data_root())
    }

    async fn resolve_egress_proxy_auth_material(
        &self,
        principal: &AuthenticatedPrincipal,
        profile: &StoredEgressProfile,
    ) -> Result<Option<DockerEgressProxyAuthMaterial>, RuntimeManagerError> {
        let Some(binding_id) = profile
            .proxy
            .as_ref()
            .and_then(|proxy| proxy.credential_binding_id)
        else {
            return Ok(None);
        };
        let Some(store) = self.session_store().await else {
            return Err(RuntimeManagerError::StartupFailed(
                "egress proxy auth requires a session store".to_string(),
            ));
        };
        let binding = store
            .get_credential_binding_for_owner(principal, binding_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?
            .ok_or_else(|| {
                RuntimeManagerError::StartupFailed(format!(
                    "egress proxy auth credential binding {binding_id} was not found"
                ))
            })?;
        let provider = self.credential_provider().await.ok_or_else(|| {
            RuntimeManagerError::StartupFailed(
                "egress proxy auth requires a configured credential provider".to_string(),
            )
        })?;
        let secret = provider
            .resolve_secret(&binding.external_ref)
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to resolve egress proxy auth credential binding {binding_id}: {error}"
                ))
            })?;
        let payload = proxy_auth_payload_from_secret(secret.payload)?;
        Ok(Some(DockerEgressProxyAuthMaterial {
            binding_id,
            target_path: self.egress_proxy_auth_config_path_for_session(),
            payload,
        }))
    }

    pub(super) async fn start_container(
        &self,
        lease: &RuntimeLease,
    ) -> Result<(), RuntimeManagerError> {
        let container_name = lease.container_name.as_deref().ok_or_else(|| {
            RuntimeManagerError::StartupFailed(
                "docker lease did not include a container name".to_string(),
            )
        })?;

        let _ = self.stop_container(container_name).await;
        let _ = remove_socket_path(&lease.agent_socket_path).await;
        let extension_dirs = self.session_extension_dirs(lease.session_id).await?;
        let launch_options = self
            .session_network_identity_launch_options(lease.session_id)
            .await?;
        log_egress_observer_launch(lease, container_name, &launch_options);
        self.initialize_session_data_volume(lease).await?;
        self.materialize_egress_custom_ca_bundle(lease.session_id, &launch_options)
            .await?;
        self.materialize_egress_proxy_auth_config(lease.session_id, &launch_options)
            .await?;
        self.materialize_session_file_bindings(lease.session_id)
            .await?;

        let mut command = Command::new(&self.config.docker_bin);
        command.args(self.docker_run_args_with_launch_options(
            lease,
            &extension_dirs,
            &launch_options,
        )?);

        let output = command.output().await.map_err(|error| {
            RuntimeManagerError::StartupFailed(format!("failed to run docker launcher: {error}"))
        })?;
        if !output.status.success() {
            return Err(RuntimeManagerError::StartupFailed(format!(
                "docker run failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        self.wait_for_socket(&lease.agent_socket_path, container_name)
            .await
    }

    async fn materialize_egress_custom_ca_bundle(
        &self,
        session_id: Uuid,
        launch_options: &DockerNetworkIdentityLaunchOptions,
    ) -> Result<(), RuntimeManagerError> {
        let Some(certificate_ref) = launch_options.egress_custom_ca_ref.as_deref() else {
            return Ok(());
        };
        let Some(target_path) = launch_options.egress_custom_ca_path.as_deref() else {
            return Err(RuntimeManagerError::StartupFailed(
                "egress TLS inspection requested a custom CA but no runtime CA target path was configured".to_string(),
            ));
        };
        let bytes = read_egress_custom_ca_bundle(certificate_ref).await?;
        if bytes.is_empty() {
            return Err(RuntimeManagerError::StartupFailed(format!(
                "egress custom CA bundle {certificate_ref} is empty"
            )));
        }
        self.write_session_data_file(session_id, target_path, "0444", &bytes)
            .await
    }

    async fn materialize_egress_proxy_auth_config(
        &self,
        session_id: Uuid,
        launch_options: &DockerNetworkIdentityLaunchOptions,
    ) -> Result<(), RuntimeManagerError> {
        let Some(material) = launch_options.egress_proxy_auth.as_ref() else {
            return Ok(());
        };
        self.write_session_data_file(session_id, &material.target_path, "0444", &material.payload)
            .await
    }

    async fn session_extension_dirs(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<String>, RuntimeManagerError> {
        let Some(store) = self.session_store().await else {
            return Ok(Vec::new());
        };
        let session = store
            .get_session_by_id(session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?
            .ok_or_else(|| {
                RuntimeManagerError::PersistenceFailed(format!(
                    "session {session_id} not found while starting docker runtime"
                ))
            })?;
        Ok(session
            .extensions
            .into_iter()
            .map(|extension| extension.install_path)
            .collect())
    }

    pub(super) async fn stop_container(
        &self,
        container_name: &str,
    ) -> Result<(), RuntimeManagerError> {
        let stop_output = Command::new(&self.config.docker_bin)
            .arg("stop")
            .arg("-t")
            .arg("20")
            .arg(container_name)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to stop docker runtime: {error}"
                ))
            })?;
        if stop_output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&stop_output.stderr);
        if stderr.contains("No such container") {
            return Ok(());
        }

        let rm_output = Command::new(&self.config.docker_bin)
            .arg("rm")
            .arg("-f")
            .arg(container_name)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to force-remove docker runtime: {error}"
                ))
            })?;
        if rm_output.status.success() {
            return Ok(());
        }

        let rm_stderr = String::from_utf8_lossy(&rm_output.stderr);
        if rm_stderr.contains("No such container") {
            return Ok(());
        }

        Err(RuntimeManagerError::StartupFailed(format!(
            "docker stop failed: {}; docker rm failed: {}",
            stderr.trim(),
            rm_stderr.trim()
        )))
    }

    async fn wait_for_socket(
        &self,
        socket_path: &str,
        container_name: &str,
    ) -> Result<(), RuntimeManagerError> {
        let deadline = Instant::now() + self.config.start_timeout;
        loop {
            if Path::new(socket_path).exists() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                let _ = self.stop_container(container_name).await;
                let _ = remove_socket_path(socket_path).await;
                return Err(RuntimeManagerError::StartupFailed(format!(
                    "docker runtime did not create socket {socket_path} before startup timeout"
                )));
            }
            sleep(Duration::from_millis(200)).await;
        }
    }

    pub(super) async fn container_exists(
        &self,
        container_name: &str,
    ) -> Result<bool, RuntimeManagerError> {
        let output = Command::new(&self.config.docker_bin)
            .arg("inspect")
            .arg("--type")
            .arg("container")
            .arg(container_name)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to inspect docker runtime {container_name}: {error}"
                ))
            })?;
        if output.status.success() {
            return Ok(true);
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("No such object") || stderr.contains("No such container") {
            return Ok(false);
        }
        Err(RuntimeManagerError::StartupFailed(format!(
            "docker inspect failed for {container_name}: {}",
            stderr.trim()
        )))
    }
}

fn push_env(env: &mut Vec<(String, String)>, key: &str, value: String) {
    if !value.is_empty() {
        env.push((key.to_string(), value));
    }
}

fn push_label(labels: &mut Vec<(String, String)>, key: &str, value: String) {
    if !value.is_empty() {
        labels.push((key.to_string(), value));
    }
}

fn log_egress_observer_launch(
    lease: &RuntimeLease,
    container_name: &str,
    launch_options: &DockerNetworkIdentityLaunchOptions,
) {
    if let Some(summary) = &launch_options.egress_observer {
        info!(
            session_id = %lease.session_id,
            container_name = container_name,
            egress_profile_id = %summary.profile_id,
            egress_profile_name = %summary.profile_name,
            egress_observation_mode = summary.observation_mode.as_str(),
            egress_proxy_configured = summary.proxy_configured,
            egress_proxy_auth_configured = summary.proxy_auth_configured,
            egress_bypass_rule_count = summary.bypass_rule_count,
            egress_custom_ca_configured = summary.custom_ca_configured,
            egress_tls_interception_enabled = summary.tls_interception_enabled,
            egress_sensitive_log_sink_configured = summary.sensitive_log_sink_configured,
            "starting docker runtime with egress observer correlation",
        );
    }
}

fn proxy_auth_payload_from_secret(payload: Value) -> Result<Vec<u8>, RuntimeManagerError> {
    let payload: EgressProxyAuthPayload = serde_json::from_value(payload).map_err(|_| {
        RuntimeManagerError::StartupFailed(
            "egress proxy auth credential payload must include username and password strings"
                .to_string(),
        )
    })?;
    if payload.username.trim().is_empty() || payload.password.trim().is_empty() {
        return Err(RuntimeManagerError::StartupFailed(
            "egress proxy auth credential payload username and password must not be empty"
                .to_string(),
        ));
    }
    if payload.username.contains(['\r', '\n']) || payload.password.contains(['\r', '\n']) {
        return Err(RuntimeManagerError::StartupFailed(
            "egress proxy auth credential payload username and password must be single-line values"
                .to_string(),
        ));
    }
    serde_json::to_vec(&payload).map_err(|error| {
        RuntimeManagerError::StartupFailed(format!(
            "failed to encode egress proxy auth credential payload: {error}"
        ))
    })
}

async fn read_egress_custom_ca_bundle(
    certificate_ref: &str,
) -> Result<Vec<u8>, RuntimeManagerError> {
    let path = local_certificate_ref_path(certificate_ref)?;
    tokio::fs::read(&path).await.map_err(|error| {
        RuntimeManagerError::StartupFailed(format!(
            "failed to read egress custom CA bundle {certificate_ref} from {}: {error}",
            path.display()
        ))
    })
}

fn local_certificate_ref_path(
    certificate_ref: &str,
) -> Result<std::path::PathBuf, RuntimeManagerError> {
    let value = certificate_ref.trim();
    let path = if let Some(file_path) = value.strip_prefix("file://") {
        Path::new(file_path)
    } else {
        Path::new(value)
    };
    if !path.is_absolute() {
        return Err(RuntimeManagerError::StartupFailed(
            "egress TLS inspection custom_ca.certificate_ref must be an absolute path or file:// path until an external CA provider is configured".to_string(),
        ));
    }
    Ok(path.to_path_buf())
}

fn posix_locale(locale: &str) -> String {
    let normalized = locale.replace('-', "_");
    if normalized.contains('.') {
        normalized
    } else {
        format!("{normalized}.UTF-8")
    }
}
