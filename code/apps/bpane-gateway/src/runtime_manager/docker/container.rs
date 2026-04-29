use std::path::Path;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::{sleep, Instant};
use uuid::Uuid;

use super::*;

impl DockerRuntimeManager {
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

        let mut command = Command::new(&self.config.docker_bin);
        command.arg("run").arg("-d").arg("--rm");
        command
            .arg("--name")
            .arg(container_name)
            .arg("--network")
            .arg(&self.config.network)
            .arg("--network-alias")
            .arg(container_name)
            .arg("-v")
            .arg(format!("{}:/run/bpane", self.config.shared_run_volume))
            .arg("--shm-size")
            .arg(&self.config.shm_size)
            .arg("--label")
            .arg(format!("browserpane.session_id={}", lease.session_id))
            .arg("-e")
            .arg(format!("BPANE_SESSION_ID={}", lease.session_id))
            .arg("-e")
            .arg(format!("BPANE_SOCKET_PATH={}", lease.agent_socket_path));
        if !extension_dirs.is_empty() {
            command
                .arg("-e")
                .arg(format!("BPANE_EXTENSION_DIRS={}", extension_dirs.join(",")));
        }

        if self.config.seccomp_unconfined {
            command.arg("--security-opt").arg("seccomp=unconfined");
        }
        if let Some(env_file) = &self.config.env_file {
            command.arg("--env-file").arg(env_file);
        }

        command.arg(&self.config.image);

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
