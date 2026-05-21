use std::path::Path;
use std::time::Duration;

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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::runtime_manager) struct DockerEgressObserverLaunchSummary {
    pub(in crate::runtime_manager) profile_id: Uuid,
    pub(in crate::runtime_manager) profile_name: String,
    pub(in crate::runtime_manager) proxy_configured: bool,
    pub(in crate::runtime_manager) bypass_rule_count: usize,
    pub(in crate::runtime_manager) custom_ca_configured: bool,
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
    ) -> DockerNetworkIdentityLaunchOptions {
        let mut env = Vec::new();
        let mut labels = Vec::new();
        let mut egress_observer = None;
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
            push_env(&mut env, "BPANE_EGRESS_PROFILE_ID", profile.id.to_string());
            push_env(&mut env, "BPANE_EGRESS_PROFILE_NAME", profile.name.clone());
            push_label(
                &mut labels,
                "browserpane.egress_profile_id",
                profile.id.to_string(),
            );
            push_label(
                &mut labels,
                "browserpane.egress_proxy_configured",
                profile.proxy.is_some().to_string(),
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
            egress_observer = Some(DockerEgressObserverLaunchSummary {
                profile_id: profile.id,
                profile_name: profile.name.clone(),
                proxy_configured: profile.proxy.is_some(),
                bypass_rule_count: profile.bypass_rules.len(),
                custom_ca_configured: profile.custom_ca.is_some(),
            });
            if let Some(proxy) = &profile.proxy {
                push_env(&mut env, "BPANE_CHROMIUM_PROXY_SERVER", proxy.url.clone());
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
            }
        }

        DockerNetworkIdentityLaunchOptions {
            env,
            labels,
            egress_observer,
        }
    }

    async fn session_network_identity_launch_options(
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
        let egress_profile = if let Some(profile_id) = session.network_identity.egress_profile_id {
            let principal = AuthenticatedPrincipal {
                subject: session.owner.subject.clone(),
                issuer: session.owner.issuer.clone(),
                display_name: session.owner.display_name.clone(),
                client_id: None,
            };
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

        Ok(Self::network_identity_launch_options(
            &session,
            egress_profile.as_ref(),
        ))
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
            egress_proxy_configured = summary.proxy_configured,
            egress_bypass_rule_count = summary.bypass_rule_count,
            egress_custom_ca_configured = summary.custom_ca_configured,
            "starting docker runtime with egress observer correlation",
        );
    }
}

fn posix_locale(locale: &str) -> String {
    let normalized = locale.replace('-', "_");
    if normalized.contains('.') {
        normalized
    } else {
        format!("{normalized}.UTF-8")
    }
}
