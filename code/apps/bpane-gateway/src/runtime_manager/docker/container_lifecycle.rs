use std::time::Duration;

use tokio::process::Command;
use tokio::time::{sleep, Instant};

use super::*;

const CONTAINER_REMOVAL_TIMEOUT: Duration = Duration::from_secs(10);
const CONTAINER_REMOVAL_POLL_INTERVAL: Duration = Duration::from_millis(100);

impl DockerRuntimeManager {
    pub(in crate::runtime_manager) async fn stop_container(
        &self,
        container_name: &str,
    ) -> Result<(), RuntimeManagerError> {
        let stop_output = Command::new(&self.config.docker_bin)
            .args(["stop", "-t", "20", container_name])
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to stop docker runtime: {error}"
                ))
            })?;
        if stop_output.status.success() {
            return self.wait_for_container_removal(container_name).await;
        }

        let stop_error = String::from_utf8_lossy(&stop_output.stderr);
        if container_is_absent(&stop_error) {
            return Ok(());
        }

        let remove_output = Command::new(&self.config.docker_bin)
            .args(["rm", "-f", container_name])
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to force-remove docker runtime: {error}"
                ))
            })?;
        let remove_error = String::from_utf8_lossy(&remove_output.stderr);
        if remove_output.status.success() || removal_is_in_progress(&remove_error) {
            return self.wait_for_container_removal(container_name).await;
        }
        if container_is_absent(&remove_error) {
            return Ok(());
        }

        Err(RuntimeManagerError::StartupFailed(format!(
            "docker stop failed: {}; docker rm failed: {}",
            stop_error.trim(),
            remove_error.trim()
        )))
    }

    async fn wait_for_container_removal(
        &self,
        container_name: &str,
    ) -> Result<(), RuntimeManagerError> {
        let deadline = Instant::now() + CONTAINER_REMOVAL_TIMEOUT;
        loop {
            if !self.container_exists(container_name).await? {
                return Ok(());
            }
            if Instant::now() >= deadline {
                return Err(RuntimeManagerError::StartupFailed(format!(
                    "docker runtime {container_name} remained after stop"
                )));
            }
            sleep(CONTAINER_REMOVAL_POLL_INTERVAL).await;
        }
    }

    pub(super) async fn container_exists(
        &self,
        container_name: &str,
    ) -> Result<bool, RuntimeManagerError> {
        let output = Command::new(&self.config.docker_bin)
            .args(["inspect", "--type", "container", container_name])
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
        let error = String::from_utf8_lossy(&output.stderr);
        if container_is_absent(&error) {
            return Ok(false);
        }
        Err(RuntimeManagerError::StartupFailed(format!(
            "docker inspect failed for {container_name}: {}",
            error.trim()
        )))
    }
}

fn container_is_absent(error: &str) -> bool {
    error.contains("No such object") || error.contains("No such container")
}

fn removal_is_in_progress(error: &str) -> bool {
    error.contains("removal of container") && error.contains("is already in progress")
}
