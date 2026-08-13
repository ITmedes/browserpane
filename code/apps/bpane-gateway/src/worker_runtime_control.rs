use std::sync::Arc;
use std::time::Duration;

use bpane_runtime_client::{
    RuntimeBrokerClient, RuntimeBrokerClientError, RuntimeBrokerClientErrorCode,
};
use bpane_runtime_contract::{
    BrokerApiVersion, ContainerLifecycleAction, ContainerLifecycleRequest, IdempotencyKey,
    RecordingWorkerLaunchRequest, RuntimeOperation, RuntimeOperationKind, RuntimeOperationRequest,
    RuntimeOperationResult, WorkerExecutionState, WorkflowWorkerLaunchRequest,
};
use uuid::Uuid;

use tokio::time::sleep;

const MAX_CONSECUTIVE_INSPECT_FAILURES: u8 = 3;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum BrokerWorkerState {
    Running,
    Exited { exit_code: Option<i32> },
    Absent,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) enum WorkerRuntimeControlErrorCode {
    Unavailable,
    Failed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub(crate) struct WorkerRuntimeControlError {
    pub(crate) code: WorkerRuntimeControlErrorCode,
}

impl std::fmt::Display for WorkerRuntimeControlError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.code {
            WorkerRuntimeControlErrorCode::Unavailable => {
                formatter.write_str("runtime broker is unavailable")
            }
            WorkerRuntimeControlErrorCode::Failed => {
                formatter.write_str("runtime broker rejected the worker operation")
            }
        }
    }
}

impl std::error::Error for WorkerRuntimeControlError {}

#[derive(Clone, Default)]
pub(crate) struct WorkerRuntimeControl {
    broker: Option<Arc<dyn RuntimeBrokerClient>>,
}

impl std::fmt::Debug for WorkerRuntimeControl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerRuntimeControl")
            .field("mode", &if self.is_broker() { "broker" } else { "direct" })
            .finish()
    }
}

impl WorkerRuntimeControl {
    #[cfg(test)]
    pub(crate) fn direct() -> Self {
        Self::default()
    }

    pub(crate) fn from_broker(broker: Option<Arc<dyn RuntimeBrokerClient>>) -> Self {
        Self { broker }
    }

    pub(crate) fn is_broker(&self) -> bool {
        self.broker.is_some()
    }

    pub(crate) async fn launch_workflow(
        &self,
        request: WorkflowWorkerLaunchRequest,
    ) -> Result<(), WorkerRuntimeControlError> {
        self.expect_result(
            RuntimeOperation::LaunchWorkflow(request),
            "workflow",
            "launch",
            |result| result == RuntimeOperationResult::Accepted,
        )
        .await
    }

    pub(crate) async fn launch_recording(
        &self,
        request: RecordingWorkerLaunchRequest,
    ) -> Result<(), WorkerRuntimeControlError> {
        self.expect_result(
            RuntimeOperation::LaunchRecording(request),
            "recording",
            "launch",
            |result| result == RuntimeOperationResult::Accepted,
        )
        .await
    }

    pub(crate) async fn inspect(
        &self,
        operation_kind: RuntimeOperationKind,
        resource_id: Uuid,
    ) -> Result<BrokerWorkerState, WorkerRuntimeControlError> {
        let result = self
            .execute(
                RuntimeOperation::ContainerLifecycle(ContainerLifecycleRequest {
                    operation_kind,
                    resource_id,
                    action: ContainerLifecycleAction::Inspect,
                }),
                family_name(operation_kind)?,
                "inspect",
            )
            .await?;
        match result {
            RuntimeOperationResult::WorkerState {
                execution_state: WorkerExecutionState::Running,
                ..
            } => Ok(BrokerWorkerState::Running),
            RuntimeOperationResult::WorkerState {
                execution_state: WorkerExecutionState::Exited,
                exit_code,
            } => Ok(BrokerWorkerState::Exited { exit_code }),
            RuntimeOperationResult::Absent => Ok(BrokerWorkerState::Absent),
            _ => Err(WorkerRuntimeControlErrorCode::Failed.into()),
        }
    }

    pub(crate) async fn remove(
        &self,
        operation_kind: RuntimeOperationKind,
        resource_id: Uuid,
    ) -> Result<(), WorkerRuntimeControlError> {
        self.expect_result(
            RuntimeOperation::ContainerLifecycle(ContainerLifecycleRequest {
                operation_kind,
                resource_id,
                action: ContainerLifecycleAction::Remove,
            }),
            family_name(operation_kind)?,
            "remove",
            |result| {
                matches!(
                    result,
                    RuntimeOperationResult::Completed { .. } | RuntimeOperationResult::Absent
                )
            },
        )
        .await
    }

    pub(crate) async fn wait_for_exit(
        &self,
        operation_kind: RuntimeOperationKind,
        resource_id: Uuid,
        poll_interval: Duration,
    ) -> Result<BrokerWorkerState, WorkerRuntimeControlError> {
        if poll_interval.is_zero() {
            return Err(WorkerRuntimeControlErrorCode::Failed.into());
        }
        let mut consecutive_failures = 0_u8;
        loop {
            match self.inspect(operation_kind, resource_id).await {
                Ok(BrokerWorkerState::Running) => consecutive_failures = 0,
                Ok(state) => return Ok(state),
                Err(error) => {
                    consecutive_failures = consecutive_failures.saturating_add(1);
                    if consecutive_failures >= MAX_CONSECUTIVE_INSPECT_FAILURES {
                        return Err(error);
                    }
                }
            }
            sleep(poll_interval).await;
        }
    }

    async fn expect_result(
        &self,
        operation: RuntimeOperation,
        family: &'static str,
        action: &'static str,
        accepted: impl FnOnce(RuntimeOperationResult) -> bool,
    ) -> Result<(), WorkerRuntimeControlError> {
        let result = self.execute(operation, family, action).await?;
        if accepted(result) {
            Ok(())
        } else {
            Err(WorkerRuntimeControlErrorCode::Failed.into())
        }
    }

    async fn execute(
        &self,
        operation: RuntimeOperation,
        family: &'static str,
        action: &'static str,
    ) -> Result<RuntimeOperationResult, WorkerRuntimeControlError> {
        let broker = self
            .broker
            .as_ref()
            .ok_or(WorkerRuntimeControlErrorCode::Failed)?;
        let request_id = Uuid::now_v7();
        let resource_id = operation.resource_id();
        let request = RuntimeOperationRequest {
            api_version: BrokerApiVersion::V1,
            request_id,
            idempotency_key: IdempotencyKey::new(format!(
                "worker:{family}:{action}:{resource_id}:{request_id}"
            ))
            .map_err(|_| WorkerRuntimeControlErrorCode::Failed)?,
            operation,
        };
        broker
            .execute(&request)
            .await
            .map(|response| response.result)
            .map_err(Into::into)
    }
}

fn family_name(
    operation_kind: RuntimeOperationKind,
) -> Result<&'static str, WorkerRuntimeControlError> {
    match operation_kind {
        RuntimeOperationKind::WorkflowWorker => Ok("workflow"),
        RuntimeOperationKind::RecordingWorker => Ok("recording"),
        RuntimeOperationKind::BrowserRuntime | RuntimeOperationKind::StorageHelper => {
            Err(WorkerRuntimeControlErrorCode::Failed.into())
        }
    }
}

impl From<WorkerRuntimeControlErrorCode> for WorkerRuntimeControlError {
    fn from(code: WorkerRuntimeControlErrorCode) -> Self {
        Self { code }
    }
}

impl From<RuntimeBrokerClientError> for WorkerRuntimeControlError {
    fn from(error: RuntimeBrokerClientError) -> Self {
        let code = match error.code {
            RuntimeBrokerClientErrorCode::Unreachable
            | RuntimeBrokerClientErrorCode::Unavailable
            | RuntimeBrokerClientErrorCode::TokenUnavailable
            | RuntimeBrokerClientErrorCode::TimedOut => WorkerRuntimeControlErrorCode::Unavailable,
            _ => WorkerRuntimeControlErrorCode::Failed,
        };
        code.into()
    }
}

#[cfg(test)]
mod tests;
