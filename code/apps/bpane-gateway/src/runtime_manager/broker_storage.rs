use std::collections::HashMap;
use std::sync::Arc;

use bpane_runtime_client::{RuntimeBrokerClient, RuntimeBrokerClientErrorCode};
use bpane_runtime_contract::{
    BrokerApiVersion, IdempotencyKey, RuntimeOperation, RuntimeOperationRequest,
    RuntimeOperationResult, SessionDataFileTarget, StorageHelperAction, StorageHelperRequest,
};
use uuid::Uuid;

use super::docker::{DockerRuntimeManager, StorageControl};
use super::{RuntimeLease, RuntimeManagerError};

impl DockerRuntimeManager {
    pub(super) async fn broker_initialize_session_data(
        &self,
        lease: &RuntimeLease,
    ) -> Result<(), RuntimeManagerError> {
        let result = self
            .execute_broker_storage(
                StorageHelperRequest {
                    action: StorageHelperAction::InitializeSessionData,
                    session_id: Some(lease.session_id),
                    source_context_id: None,
                    target_context_id: lease.browser_context_id,
                    file_target: None,
                    declared_payload_bytes: None,
                },
                None,
            )
            .await?;
        require_completed(
            result.response.result,
            result.payload,
            "initialize session data",
        )
    }

    pub(super) async fn broker_delete_session_data(
        &self,
        session_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        self.broker_delete(
            StorageHelperRequest {
                action: StorageHelperAction::DeleteSessionData,
                session_id: Some(session_id),
                source_context_id: None,
                target_context_id: None,
                file_target: None,
                declared_payload_bytes: None,
            },
            "delete session data",
        )
        .await
    }

    pub(super) async fn broker_delete_browser_context(
        &self,
        context_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        self.broker_delete(
            StorageHelperRequest {
                action: StorageHelperAction::DeleteBrowserContext,
                session_id: None,
                source_context_id: Some(context_id),
                target_context_id: None,
                file_target: None,
                declared_payload_bytes: None,
            },
            "delete browser context",
        )
        .await
    }

    async fn broker_delete(
        &self,
        request: StorageHelperRequest,
        operation_name: &'static str,
    ) -> Result<(), RuntimeManagerError> {
        let result = self.execute_broker_storage(request, None).await?;
        if matches!(
            result.response.result,
            RuntimeOperationResult::Completed { .. } | RuntimeOperationResult::Absent
        ) && result.payload.is_none()
        {
            Ok(())
        } else {
            Err(invalid_result(operation_name))
        }
    }

    pub(super) async fn broker_clone_browser_context(
        &self,
        source_context_id: Uuid,
        target_context_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        let result = self
            .execute_broker_storage(
                StorageHelperRequest {
                    action: StorageHelperAction::CloneBrowserContext,
                    session_id: None,
                    source_context_id: Some(source_context_id),
                    target_context_id: Some(target_context_id),
                    file_target: None,
                    declared_payload_bytes: None,
                },
                None,
            )
            .await?;
        if matches!(
            result.response.result,
            RuntimeOperationResult::Completed { .. } | RuntimeOperationResult::Absent
        ) && result.payload.is_none()
        {
            Ok(())
        } else {
            Err(invalid_result("clone browser context"))
        }
    }

    pub(super) async fn broker_export_browser_context(
        &self,
        context_id: Uuid,
    ) -> Result<Option<Vec<u8>>, RuntimeManagerError> {
        let result = self
            .execute_broker_storage(
                StorageHelperRequest {
                    action: StorageHelperAction::ExportBrowserContext,
                    session_id: None,
                    source_context_id: Some(context_id),
                    target_context_id: None,
                    file_target: None,
                    declared_payload_bytes: None,
                },
                None,
            )
            .await?;
        match (result.response.result, result.payload) {
            (RuntimeOperationResult::StoragePayload { .. }, Some(payload)) => Ok(Some(payload)),
            (RuntimeOperationResult::Absent, None) => Ok(None),
            _ => Err(invalid_result("export browser context")),
        }
    }

    pub(super) async fn broker_import_browser_context(
        &self,
        context_id: Uuid,
        profile_archive: Option<&[u8]>,
    ) -> Result<(), RuntimeManagerError> {
        let Some(profile_archive) = profile_archive else {
            return self.broker_delete_browser_context(context_id).await;
        };
        let payload_bytes = u64::try_from(profile_archive.len())
            .map_err(|_| invalid_request("import browser context"))?;
        let result = self
            .execute_broker_storage(
                StorageHelperRequest {
                    action: StorageHelperAction::ImportBrowserContext,
                    session_id: None,
                    source_context_id: None,
                    target_context_id: Some(context_id),
                    file_target: None,
                    declared_payload_bytes: Some(payload_bytes),
                },
                Some(profile_archive),
            )
            .await?;
        require_completed(
            result.response.result,
            result.payload,
            "import browser context",
        )
    }

    pub(super) async fn broker_measure_browser_contexts(
        &self,
        context_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, u64>, RuntimeManagerError> {
        let mut usage = HashMap::with_capacity(context_ids.len());
        for context_id in context_ids {
            let result = self
                .execute_broker_storage(
                    StorageHelperRequest {
                        action: StorageHelperAction::MeasureBrowserContext,
                        session_id: None,
                        source_context_id: Some(*context_id),
                        target_context_id: None,
                        file_target: None,
                        declared_payload_bytes: None,
                    },
                    None,
                )
                .await?;
            let bytes = match (result.response.result, result.payload) {
                (RuntimeOperationResult::StorageUsage { storage_bytes }, None) => storage_bytes,
                (RuntimeOperationResult::Absent, None) => 0,
                _ => return Err(invalid_result("measure browser context")),
            };
            usage.insert(*context_id, bytes);
        }
        Ok(usage)
    }

    pub(super) async fn broker_write_session_data(
        &self,
        session_id: Uuid,
        target: SessionDataFileTarget,
        bytes: &[u8],
    ) -> Result<(), RuntimeManagerError> {
        let payload_bytes =
            u64::try_from(bytes.len()).map_err(|_| invalid_request("materialize session data"))?;
        let result = self
            .execute_broker_storage(
                StorageHelperRequest {
                    action: StorageHelperAction::MaterializeSessionFiles,
                    session_id: Some(session_id),
                    source_context_id: None,
                    target_context_id: None,
                    file_target: Some(target),
                    declared_payload_bytes: Some(payload_bytes),
                },
                Some(bytes),
            )
            .await?;
        require_completed(
            result.response.result,
            result.payload,
            "materialize session data",
        )
    }

    async fn execute_broker_storage(
        &self,
        storage: StorageHelperRequest,
        payload: Option<&[u8]>,
    ) -> Result<bpane_runtime_client::RuntimeStorageOperationResponse, RuntimeManagerError> {
        let StorageControl::Broker(client) = &self.storage_control else {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "runtime broker storage operation requires broker storage control".to_string(),
            ));
        };
        execute_storage(client, storage, payload).await
    }
}

async fn execute_storage(
    client: &Arc<dyn RuntimeBrokerClient>,
    storage: StorageHelperRequest,
    payload: Option<&[u8]>,
) -> Result<bpane_runtime_client::RuntimeStorageOperationResponse, RuntimeManagerError> {
    let request_id = Uuid::now_v7();
    let action = storage_action_name(storage.action);
    let resource_id = storage
        .session_id
        .or(storage.target_context_id)
        .or(storage.source_context_id)
        .ok_or_else(|| invalid_request(action))?;
    let request = RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id,
        idempotency_key: IdempotencyKey::new(format!(
            "storage:{action}:{resource_id}:{request_id}"
        ))
        .map_err(|_| invalid_request(action))?,
        operation: RuntimeOperation::RunStorageHelper(storage),
    };
    client
        .execute_storage(&request, payload)
        .await
        .map_err(|error| match error.code {
            RuntimeBrokerClientErrorCode::Unreachable
            | RuntimeBrokerClientErrorCode::Unavailable
            | RuntimeBrokerClientErrorCode::TokenUnavailable
            | RuntimeBrokerClientErrorCode::TimedOut => {
                RuntimeManagerError::Unavailable(error.to_string())
            }
            _ => RuntimeManagerError::StartupFailed(error.to_string()),
        })
}

fn require_completed(
    result: RuntimeOperationResult,
    payload: Option<Vec<u8>>,
    operation_name: &'static str,
) -> Result<(), RuntimeManagerError> {
    if matches!(result, RuntimeOperationResult::Completed { .. }) && payload.is_none() {
        Ok(())
    } else {
        Err(invalid_result(operation_name))
    }
}

fn storage_action_name(action: StorageHelperAction) -> &'static str {
    match action {
        StorageHelperAction::InitializeSessionData => "initialize-session-data",
        StorageHelperAction::MaterializeSessionFiles => "materialize-session-files",
        StorageHelperAction::DeleteSessionData => "delete-session-data",
        StorageHelperAction::CloneBrowserContext => "clone-browser-context",
        StorageHelperAction::ExportBrowserContext => "export-browser-context",
        StorageHelperAction::ImportBrowserContext => "import-browser-context",
        StorageHelperAction::MeasureBrowserContext => "measure-browser-context",
        StorageHelperAction::DeleteBrowserContext => "delete-browser-context",
    }
}

fn invalid_request(operation_name: &str) -> RuntimeManagerError {
    RuntimeManagerError::InvalidConfiguration(format!(
        "runtime broker {operation_name} request could not be represented safely"
    ))
}

fn invalid_result(operation_name: &str) -> RuntimeManagerError {
    RuntimeManagerError::StartupFailed(format!(
        "runtime broker returned an invalid {operation_name} result"
    ))
}
