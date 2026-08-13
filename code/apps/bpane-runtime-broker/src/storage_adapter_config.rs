use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context};
use bpane_runtime_contract::ResourceLimits;

use crate::executor::StorageRoutedRuntimeExecutor;
use crate::{RuntimeOperationExecutor, StorageRuntimeDockerAdapter, StorageRuntimeDockerConfig};

#[derive(Debug, Clone)]
pub struct StorageAdapterSettings {
    pub image: Option<String>,
    pub session_data_volume_prefix: String,
    pub browser_context_volume_prefix: String,
    pub container_name_prefix: String,
    pub max_payload_bytes: usize,
    pub max_archive_entries: usize,
    pub max_archive_path_bytes: usize,
    pub max_archive_uncompressed_bytes: u64,
}

impl StorageAdapterSettings {
    pub fn combine_executor(
        &self,
        primary: Arc<dyn RuntimeOperationExecutor>,
        docker_api_url: Option<&str>,
        docker_timeout_secs: u64,
    ) -> anyhow::Result<Arc<dyn RuntimeOperationExecutor>> {
        let Some(image) = self
            .image
            .as_deref()
            .filter(|value| !value.trim().is_empty())
        else {
            return Ok(primary);
        };
        let docker_api_url = docker_api_url
            .context("Docker API URL is required when storage helpers are enabled")?;
        let timeout = Duration::from_secs(docker_timeout_secs);
        if timeout.is_zero() || timeout > Duration::from_secs(300) {
            bail!("storage helper Docker timeout must be between 1 and 300 seconds");
        }
        let output_limit_bytes = u64::try_from(self.max_payload_bytes)
            .context("storage helper payload limit is invalid")?;
        let config = StorageRuntimeDockerConfig {
            image: image.to_string(),
            session_data_volume_prefix: self.session_data_volume_prefix.clone(),
            browser_context_volume_prefix: self.browser_context_volume_prefix.clone(),
            container_name_prefix: self.container_name_prefix.clone(),
            seccomp_profile: "default".to_string(),
            resources: ResourceLimits {
                memory_bytes: 512 * 1024 * 1024,
                cpu_millis: 1_000,
                pids: 128,
                shm_bytes: 16 * 1024 * 1024,
                timeout_secs: docker_timeout_secs,
                output_limit_bytes,
            },
            max_payload_bytes: self.max_payload_bytes,
            max_archive_entries: self.max_archive_entries,
            max_archive_path_bytes: self.max_archive_path_bytes,
            max_archive_uncompressed_bytes: self.max_archive_uncompressed_bytes,
        };
        let storage: Arc<dyn RuntimeOperationExecutor> = Arc::new(
            StorageRuntimeDockerAdapter::connect(config, docker_api_url, timeout).map_err(
                |_| anyhow::anyhow!("storage helper Docker adapter configuration failed"),
            )?,
        );
        Ok(Arc::new(StorageRoutedRuntimeExecutor::new(
            primary, storage,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RejectingRuntimeExecutor;

    fn settings() -> StorageAdapterSettings {
        StorageAdapterSettings {
            image: None,
            session_data_volume_prefix: "bpane-session-data".to_string(),
            browser_context_volume_prefix: "bpane-browser-context".to_string(),
            container_name_prefix: "bpane-storage-helper".to_string(),
            max_payload_bytes: 1024,
            max_archive_entries: 100,
            max_archive_path_bytes: 256,
            max_archive_uncompressed_bytes: 4096,
        }
    }

    #[test]
    fn leaves_primary_executor_unchanged_when_disabled() {
        settings()
            .combine_executor(Arc::new(RejectingRuntimeExecutor), None, 30)
            .unwrap();
    }

    #[test]
    fn rejects_partial_or_mutable_storage_configuration() {
        let mut configured = settings();
        configured.image = Some("browser:latest".to_string());
        assert!(configured
            .combine_executor(Arc::new(RejectingRuntimeExecutor), None, 30)
            .is_err());
        assert!(configured
            .combine_executor(
                Arc::new(RejectingRuntimeExecutor),
                Some("http://docker-proxy:2375"),
                30,
            )
            .is_err());
    }
}
