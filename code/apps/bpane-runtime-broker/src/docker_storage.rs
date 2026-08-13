use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bpane_runtime_contract::{
    RuntimeBrokerPolicy, RuntimeOperation, RuntimeOperationRequest, RuntimeOperationResult,
    StorageHelperAction,
};

use crate::{ExecutionError, ExecutionErrorCode, RuntimeOperationExecutor, StorageExecutionOutput};

mod archive;
mod backend;
mod config;
mod materialize;

pub use config::StorageRuntimeDockerConfig;

use archive::validate_context_archive;
use backend::{BollardStorageDockerApi, StorageDockerApi, StorageDockerError};
use materialize::MaterializedStorageHelper;

const STORAGE_INPUT_MOUNT_ROOT: &str = "/run/bpane/storage-helper/input";
const STORAGE_HELPER_SCRIPT: &str = r#"set -eu; clean_dir() { find "$1" -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +; }; read_input() { test "$(wc -c < /run/bpane/storage-helper/input/payload)" -eq "${BPANE_STORAGE_INPUT_BYTES:?}"; cat /run/bpane/storage-helper/input/payload; }; case "${BPANE_STORAGE_ACTION:?}" in initialize_session_data) mkdir -p "$BPANE_SESSION_DATA_DIR" "$BPANE_PROFILE_DIR" "$BPANE_UPLOAD_DIR" "$BPANE_DOWNLOAD_DIR" "$BPANE_SESSION_FILE_MOUNTS_DIR"; chmod 0770 "$BPANE_SESSION_DATA_DIR" "$BPANE_PROFILE_DIR" "$BPANE_UPLOAD_DIR" "$BPANE_DOWNLOAD_DIR" "$BPANE_SESSION_FILE_MOUNTS_DIR" ;; materialize_session_files) parent=$(dirname "$BPANE_MATERIALIZE_TARGET"); mkdir -p "$parent"; temporary="${BPANE_MATERIALIZE_TARGET}.tmp.$$"; trap 'rm -f "$temporary"' EXIT; read_input > "$temporary"; chmod "$BPANE_MATERIALIZE_MODE" "$temporary"; mv -f "$temporary" "$BPANE_MATERIALIZE_TARGET"; trap - EXIT ;; clone_browser_context) clean_dir /run/bpane/storage-helper/target; cp -a /run/bpane/storage-helper/source/. /run/bpane/storage-helper/target/; chmod 0770 /run/bpane/storage-helper/target ;; export_browser_context) cd /run/bpane/storage-helper/source; find . -xdev \( -type f -o -type d \) -print0 | tar --null --no-recursion --hard-dereference --files-from=- -czf - ;; import_browser_context) clean_dir /run/bpane/storage-helper/target; read_input | tar -C /run/bpane/storage-helper/target -xzf - --no-same-owner --no-same-permissions --delay-directory-restore; if find /run/bpane/storage-helper/target -xdev ! -type d ! -type f -print -quit | grep -q .; then exit 65; fi; chmod 0770 /run/bpane/storage-helper/target ;; measure_browser_context) du -sb /run/bpane/storage-helper/source | cut -f1 ;; *) exit 64 ;; esac"#;

/// Policy-validating Docker adapter for bounded storage operations.
pub struct StorageRuntimeDockerAdapter {
    config: StorageRuntimeDockerConfig,
    policy: RuntimeBrokerPolicy,
    backend: Arc<dyn StorageDockerApi>,
}

impl std::fmt::Debug for StorageRuntimeDockerAdapter {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("StorageRuntimeDockerAdapter")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

impl StorageRuntimeDockerAdapter {
    /// Connects the storage adapter to a private HTTP Docker API endpoint.
    ///
    /// # Errors
    ///
    /// Returns a sanitized failure for invalid trusted configuration.
    pub fn connect(
        config: StorageRuntimeDockerConfig,
        docker_api_url: &str,
        timeout: Duration,
    ) -> Result<Self, ExecutionError> {
        let backend = Arc::new(BollardStorageDockerApi::connect(docker_api_url, timeout)?);
        Self::with_backend(config, backend)
    }

    fn with_backend(
        config: StorageRuntimeDockerConfig,
        backend: Arc<dyn StorageDockerApi>,
    ) -> Result<Self, ExecutionError> {
        let policy = config
            .build_policy()
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        Ok(Self {
            config,
            policy,
            backend,
        })
    }

    async fn execute_helper(
        &self,
        request_id: uuid::Uuid,
        request: &bpane_runtime_contract::StorageHelperRequest,
        payload: Option<&[u8]>,
    ) -> Result<StorageExecutionOutput, ExecutionError> {
        if request.action == StorageHelperAction::ImportBrowserContext {
            validate_context_archive(
                payload.ok_or(ExecutionErrorCode::AdapterFailed)?,
                &self.config,
            )
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        }
        let source_exists = self.source_exists(request).await?;
        if !source_exists
            && matches!(
                request.action,
                StorageHelperAction::CloneBrowserContext
                    | StorageHelperAction::ExportBrowserContext
                    | StorageHelperAction::MeasureBrowserContext
            )
        {
            return Ok(absent_output());
        }
        let cleanup_target = self.prepare_target(request).await?;
        let helper = MaterializedStorageHelper::new(&self.config, request_id, request)
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        if let Some(input_volume) = helper.input_volume.as_deref() {
            self.remove_owned_volume_if_present(input_volume).await?;
        }
        self.policy
            .authorize_launch(&helper.policy_spec)
            .map_err(|_| ExecutionErrorCode::AdapterFailed)?;
        let action = helper.action;
        let input_volume = helper.input_volume.clone();
        let output = self
            .backend
            .run_helper(
                helper.container_name,
                helper.container,
                payload.map(ToOwned::to_owned),
                helper.output_limit,
            )
            .await;
        let input_cleanup = if let Some(volume) = input_volume.as_deref() {
            self.remove_owned_volume_if_present(volume).await
        } else {
            Ok(())
        };
        if output.is_err() || input_cleanup.is_err() {
            if let Some(volume) = cleanup_target {
                let _ = self.backend.remove_volume(&volume).await;
            }
            return Err(ExecutionErrorCode::AdapterFailed.into());
        }
        let output = match output {
            Ok(output) => output,
            Err(error) => return Err(map_backend_error(error)),
        };
        MaterializedStorageHelper::result(action, output)
            .map_err(|_| ExecutionErrorCode::AdapterFailed.into())
    }

    async fn source_exists(
        &self,
        request: &bpane_runtime_contract::StorageHelperRequest,
    ) -> Result<bool, ExecutionError> {
        let source_id = match request.action {
            StorageHelperAction::CloneBrowserContext
            | StorageHelperAction::ExportBrowserContext
            | StorageHelperAction::MeasureBrowserContext => request.source_context_id,
            _ => None,
        };
        let Some(source_id) = source_id else {
            return Ok(false);
        };
        self.backend
            .volume_exists(&self.config.context_volume(source_id))
            .await
            .map_err(map_backend_error)
    }

    async fn prepare_target(
        &self,
        request: &bpane_runtime_contract::StorageHelperRequest,
    ) -> Result<Option<String>, ExecutionError> {
        let target_id = match request.action {
            StorageHelperAction::CloneBrowserContext
            | StorageHelperAction::ImportBrowserContext => request.target_context_id,
            _ => None,
        };
        let Some(target_id) = target_id else {
            return Ok(None);
        };
        let volume = self.config.context_volume(target_id);
        match self.backend.remove_volume(&volume).await {
            Ok(()) | Err(StorageDockerError::NotFound) => Ok(Some(volume)),
            Err(error) => Err(map_backend_error(error)),
        }
    }

    async fn delete_volume(
        &self,
        volume_name: String,
    ) -> Result<StorageExecutionOutput, ExecutionError> {
        let result = match self.backend.remove_volume(&volume_name).await {
            Ok(()) => RuntimeOperationResult::Completed {
                exit_code: None,
                omitted_output_bytes: 0,
            },
            Err(StorageDockerError::NotFound) => RuntimeOperationResult::Absent,
            Err(error) => return Err(map_backend_error(error)),
        };
        Ok(StorageExecutionOutput {
            result,
            payload: None,
        })
    }

    async fn remove_owned_volume_if_present(&self, volume: &str) -> Result<(), ExecutionError> {
        match self.backend.remove_volume(volume).await {
            Ok(()) | Err(StorageDockerError::NotFound) => Ok(()),
            Err(error) => Err(map_backend_error(error)),
        }
    }
}

#[async_trait]
impl RuntimeOperationExecutor for StorageRuntimeDockerAdapter {
    async fn check_readiness(&self) -> Result<(), ExecutionError> {
        self.backend.ping().await.map_err(map_backend_error)
    }

    async fn execute(
        &self,
        _request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResult, ExecutionError> {
        Err(ExecutionErrorCode::AdapterUnavailable.into())
    }

    async fn execute_storage(
        &self,
        request: &RuntimeOperationRequest,
        payload: Option<&[u8]>,
    ) -> Result<StorageExecutionOutput, ExecutionError> {
        let RuntimeOperation::RunStorageHelper(storage) = &request.operation else {
            return Err(ExecutionErrorCode::AdapterUnavailable.into());
        };
        match storage.action {
            StorageHelperAction::DeleteSessionData => {
                self.delete_volume(
                    self.config.session_data_volume(
                        storage
                            .session_id
                            .ok_or(ExecutionErrorCode::AdapterFailed)?,
                    ),
                )
                .await
            }
            StorageHelperAction::DeleteBrowserContext => {
                self.delete_volume(
                    self.config.context_volume(
                        storage
                            .source_context_id
                            .ok_or(ExecutionErrorCode::AdapterFailed)?,
                    ),
                )
                .await
            }
            _ => {
                self.execute_helper(request.request_id, storage, payload)
                    .await
            }
        }
    }
}

fn absent_output() -> StorageExecutionOutput {
    StorageExecutionOutput {
        result: RuntimeOperationResult::Absent,
        payload: None,
    }
}

fn map_backend_error(_error: StorageDockerError) -> ExecutionError {
    ExecutionErrorCode::AdapterFailed.into()
}

#[cfg(test)]
mod tests;
