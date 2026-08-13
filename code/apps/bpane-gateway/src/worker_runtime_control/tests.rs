use std::sync::Mutex;

use async_trait::async_trait;
use bpane_runtime_client::RuntimeBrokerClientError;
use bpane_runtime_contract::{
    RecordingWorkerCredentials, RuntimeOperationResponse, SecretValue, WorkflowWorkerCredentials,
};

use super::*;

struct FakeBrokerClient {
    requests: Mutex<Vec<RuntimeOperationRequest>>,
    result: Mutex<Result<RuntimeOperationResult, RuntimeBrokerClientErrorCode>>,
}

impl FakeBrokerClient {
    fn returning(result: RuntimeOperationResult) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            result: Mutex::new(Ok(result)),
        }
    }

    fn failing(code: RuntimeBrokerClientErrorCode) -> Self {
        Self {
            requests: Mutex::new(Vec::new()),
            result: Mutex::new(Err(code)),
        }
    }
}

#[async_trait]
impl RuntimeBrokerClient for FakeBrokerClient {
    async fn check_readiness(&self) -> Result<(), RuntimeBrokerClientError> {
        Ok(())
    }

    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResponse, RuntimeBrokerClientError> {
        self.requests.lock().unwrap().push(request.clone());
        match self.result.lock().unwrap().clone() {
            Ok(result) => Ok(RuntimeOperationResponse {
                api_version: BrokerApiVersion::V1,
                request_id: request.request_id,
                result,
            }),
            Err(code) => Err(code.into()),
        }
    }
}

fn secret(value: &str) -> SecretValue {
    SecretValue::new(value).unwrap()
}

fn workflow_request(run_id: Uuid) -> WorkflowWorkerLaunchRequest {
    WorkflowWorkerLaunchRequest {
        workflow_run_id: run_id,
        session_id: Uuid::now_v7(),
        automation_task_id: Uuid::now_v7(),
        credentials: WorkflowWorkerCredentials {
            session_automation_access_token: secret("automation-secret"),
            gateway_bearer_token: Some(secret("gateway-secret")),
        },
    }
}

fn recording_request(recording_id: Uuid) -> RecordingWorkerLaunchRequest {
    RecordingWorkerLaunchRequest {
        session_id: Uuid::now_v7(),
        recording_id,
        credentials: RecordingWorkerCredentials {
            connect_ticket: secret("connect-secret"),
            session_automation_access_token: secret("automation-secret"),
            recording_worker_access_token: secret("worker-secret"),
            gateway_bearer_token: Some(secret("gateway-secret")),
        },
    }
}

#[tokio::test]
async fn launches_both_worker_families_through_typed_requests() {
    let client = Arc::new(FakeBrokerClient::returning(
        RuntimeOperationResult::Accepted,
    ));
    let broker: Arc<dyn RuntimeBrokerClient> = client.clone();
    let control = WorkerRuntimeControl::from_broker(Some(broker));
    let run_id = Uuid::now_v7();
    let recording_id = Uuid::now_v7();

    control
        .launch_workflow(workflow_request(run_id))
        .await
        .unwrap();
    control
        .launch_recording(recording_request(recording_id))
        .await
        .unwrap();

    let requests = client.requests.lock().unwrap();
    assert!(matches!(
        &requests[0].operation,
        RuntimeOperation::LaunchWorkflow(request) if request.workflow_run_id == run_id
    ));
    assert!(matches!(
        &requests[1].operation,
        RuntimeOperation::LaunchRecording(request) if request.recording_id == recording_id
    ));
    let debug = format!("{requests:?}");
    for secret in [
        "connect-secret",
        "automation-secret",
        "worker-secret",
        "gateway-secret",
    ] {
        assert!(!debug.contains(secret));
    }
}

#[tokio::test]
async fn maps_worker_inspect_and_remove_results() {
    let client = Arc::new(FakeBrokerClient::returning(
        RuntimeOperationResult::WorkerState {
            execution_state: WorkerExecutionState::Exited,
            exit_code: Some(9),
        },
    ));
    let broker: Arc<dyn RuntimeBrokerClient> = client.clone();
    let control = WorkerRuntimeControl::from_broker(Some(broker));
    let resource_id = Uuid::now_v7();

    assert_eq!(
        control
            .inspect(RuntimeOperationKind::WorkflowWorker, resource_id)
            .await
            .unwrap(),
        BrokerWorkerState::Exited { exit_code: Some(9) }
    );
    *client.result.lock().unwrap() = Ok(RuntimeOperationResult::Absent);
    control
        .remove(RuntimeOperationKind::WorkflowWorker, resource_id)
        .await
        .unwrap();

    let requests = client.requests.lock().unwrap();
    assert!(matches!(
        &requests[0].operation,
        RuntimeOperation::ContainerLifecycle(request)
            if request.action == ContainerLifecycleAction::Inspect
    ));
    assert!(matches!(
        &requests[1].operation,
        RuntimeOperation::ContainerLifecycle(request)
            if request.action == ContainerLifecycleAction::Remove
    ));
}

#[tokio::test]
async fn rejects_invalid_results_direct_mode_and_unavailable_broker() {
    let invalid = Arc::new(FakeBrokerClient::returning(RuntimeOperationResult::Exists));
    let broker: Arc<dyn RuntimeBrokerClient> = invalid;
    let control = WorkerRuntimeControl::from_broker(Some(broker));
    assert_eq!(
        control
            .launch_workflow(workflow_request(Uuid::now_v7()))
            .await
            .unwrap_err()
            .code,
        WorkerRuntimeControlErrorCode::Failed
    );

    let direct = WorkerRuntimeControl::direct();
    assert!(!direct.is_broker());
    assert_eq!(
        direct
            .inspect(RuntimeOperationKind::WorkflowWorker, Uuid::now_v7())
            .await
            .unwrap_err()
            .code,
        WorkerRuntimeControlErrorCode::Failed
    );

    let unavailable = Arc::new(FakeBrokerClient::failing(
        RuntimeBrokerClientErrorCode::Unreachable,
    ));
    let broker: Arc<dyn RuntimeBrokerClient> = unavailable;
    let control = WorkerRuntimeControl::from_broker(Some(broker));
    assert_eq!(
        control
            .launch_recording(recording_request(Uuid::now_v7()))
            .await
            .unwrap_err()
            .code,
        WorkerRuntimeControlErrorCode::Unavailable
    );
}

#[tokio::test]
async fn worker_monitor_bounds_broker_failures_and_rejects_busy_polling() {
    let unavailable = Arc::new(FakeBrokerClient::failing(
        RuntimeBrokerClientErrorCode::Unreachable,
    ));
    let broker: Arc<dyn RuntimeBrokerClient> = unavailable.clone();
    let control = WorkerRuntimeControl::from_broker(Some(broker));

    assert_eq!(
        control
            .wait_for_exit(
                RuntimeOperationKind::WorkflowWorker,
                Uuid::now_v7(),
                Duration::from_millis(1),
            )
            .await
            .unwrap_err()
            .code,
        WorkerRuntimeControlErrorCode::Unavailable
    );
    assert_eq!(unavailable.requests.lock().unwrap().len(), 3);
    assert_eq!(
        control
            .wait_for_exit(
                RuntimeOperationKind::WorkflowWorker,
                Uuid::now_v7(),
                Duration::ZERO,
            )
            .await
            .unwrap_err()
            .code,
        WorkerRuntimeControlErrorCode::Failed
    );
}
