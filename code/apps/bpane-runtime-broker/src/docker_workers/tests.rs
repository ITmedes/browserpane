use std::collections::{BTreeMap, VecDeque};
use std::sync::Mutex;

use bollard::models::ContainerCreateBody;
use bpane_runtime_contract::{
    BrokerApiVersion, ContainerLifecycleAction, ContainerLifecycleRequest, IdempotencyKey,
    RecordingWorkerCredentials, RecordingWorkerLaunchRequest, ResourceLimits, RuntimeOperation,
    RuntimeOperationKind, RuntimeOperationRequest, RuntimeOperationResult, SecretValue,
    WorkerExecutionState, WorkflowWorkerCredentials, WorkflowWorkerLaunchRequest,
};
use uuid::Uuid;

use super::*;

#[derive(Debug)]
enum BackendCall {
    Ping,
    Create {
        name: String,
        body: Box<ContainerCreateBody>,
    },
    Start(String),
    SendStdin {
        name: String,
        contents: Vec<u8>,
    },
    Inspect,
    Stop,
    Remove,
}

struct FakeDockerBackend {
    calls: Mutex<Vec<BackendCall>>,
    failures: Mutex<VecDeque<(&'static str, DockerBackendError)>>,
    state: Mutex<Option<DockerContainerState>>,
}

impl Default for FakeDockerBackend {
    fn default() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            failures: Mutex::new(VecDeque::new()),
            state: Mutex::new(None),
        }
    }
}

impl FakeDockerBackend {
    fn with_state(state: DockerContainerState) -> Self {
        Self {
            state: Mutex::new(Some(state)),
            ..Self::default()
        }
    }

    fn set_state(&self, state: DockerContainerState) {
        *self.state.lock().unwrap() = Some(state);
    }

    fn fail_next(&self, operation: &'static str, error: DockerBackendError) {
        self.failures.lock().unwrap().push_back((operation, error));
    }

    fn failure(&self, operation: &str) -> Option<DockerBackendError> {
        let mut failures = self.failures.lock().unwrap();
        if failures.front().is_some_and(|(name, _)| *name == operation) {
            failures.pop_front().map(|(_, error)| error)
        } else {
            None
        }
    }
}

#[async_trait]
impl DockerContainerApi for FakeDockerBackend {
    async fn ping(&self) -> Result<(), DockerBackendError> {
        self.calls.lock().unwrap().push(BackendCall::Ping);
        self.failure("ping").map_or(Ok(()), Err)
    }

    async fn create(
        &self,
        name: &str,
        body: ContainerCreateBody,
    ) -> Result<(), DockerBackendError> {
        self.calls.lock().unwrap().push(BackendCall::Create {
            name: name.to_string(),
            body: Box::new(body),
        });
        if let Some(error) = self.failure("create") {
            return Err(error);
        }
        self.set_state(DockerContainerState::Running);
        Ok(())
    }

    async fn start(&self, name: &str) -> Result<(), DockerBackendError> {
        self.calls
            .lock()
            .unwrap()
            .push(BackendCall::Start(name.to_string()));
        self.failure("start").map_or(Ok(()), Err)
    }

    async fn send_stdin(&self, name: &str, contents: Vec<u8>) -> Result<(), DockerBackendError> {
        self.calls.lock().unwrap().push(BackendCall::SendStdin {
            name: name.to_string(),
            contents,
        });
        self.failure("send_stdin").map_or(Ok(()), Err)
    }

    async fn inspect(&self, name: &str) -> Result<DockerContainerState, DockerBackendError> {
        let _ = name;
        self.calls.lock().unwrap().push(BackendCall::Inspect);
        if let Some(error) = self.failure("inspect") {
            return Err(error);
        }
        self.state
            .lock()
            .unwrap()
            .ok_or(DockerBackendError::NotFound)
    }

    async fn stop(&self, name: &str) -> Result<(), DockerBackendError> {
        let _ = name;
        self.calls.lock().unwrap().push(BackendCall::Stop);
        if let Some(error) = self.failure("stop") {
            return Err(error);
        }
        self.state
            .lock()
            .unwrap()
            .take()
            .map(|_| ())
            .ok_or(DockerBackendError::NotFound)
    }

    async fn remove(&self, name: &str) -> Result<(), DockerBackendError> {
        let _ = name;
        self.calls.lock().unwrap().push(BackendCall::Remove);
        if let Some(error) = self.failure("remove") {
            return Err(error);
        }
        self.state
            .lock()
            .unwrap()
            .take()
            .map(|_| ())
            .ok_or(DockerBackendError::NotFound)
    }
}

fn limits() -> ResourceLimits {
    ResourceLimits {
        memory_bytes: 512 * 1024 * 1024,
        cpu_millis: 1_000,
        pids: 256,
        shm_bytes: 128 * 1024 * 1024,
        timeout_secs: 600,
        output_limit_bytes: 262_144,
    }
}

fn oidc() -> WorkerOidcConfig {
    WorkerOidcConfig {
        token_url: "http://keycloak:8080/realms/browserpane/protocol/openid-connect/token"
            .to_string(),
        client_id: "bpane-worker".to_string(),
        client_secret: secret("oidc-secret"),
        scopes: String::new(),
    }
}

fn workflow_config() -> WorkflowWorkerDockerConfig {
    WorkflowWorkerDockerConfig {
        image: format!(
            "registry.example/browserpane/workflow-worker@sha256:{}",
            "a".repeat(64)
        ),
        network: "bpane-internal".to_string(),
        container_name_prefix: "bpane-workflow".to_string(),
        gateway_api_url: "http://gateway:8932".to_string(),
        work_root: "/tmp/bpane-workflows".to_string(),
        request_timeout_ms: 30_000,
        output_limit_bytes: 262_144,
        command: vec![
            "/usr/local/bin/npx".to_string(),
            "tsx".to_string(),
            "src/index.ts".to_string(),
        ],
        seccomp_profile: "default".to_string(),
        resources: limits(),
        oidc: Some(oidc()),
    }
}

fn recording_config() -> RecordingWorkerDockerConfig {
    RecordingWorkerDockerConfig {
        image: format!(
            "registry.example/browserpane/recording-worker@sha256:{}",
            "b".repeat(64)
        ),
        network: "bpane-internal".to_string(),
        container_name_prefix: "bpane-recording".to_string(),
        artifact_volume: "bpane-recordings".to_string(),
        chrome_executable: "/usr/bin/chromium".to_string(),
        gateway_api_url: "http://gateway:8932".to_string(),
        page_url: "http://web:8080/recording-worker.html".to_string(),
        connect_gateway_url: "https://gateway:4433".to_string(),
        output_root: "/tmp/bpane-recordings".to_string(),
        cert_spki: Some("sha256/test-spki".to_string()),
        headless: true,
        connect_timeout_ms: 120_000,
        poll_interval_ms: 250,
        request_timeout_ms: 30_000,
        command: vec![
            "/usr/local/bin/npx".to_string(),
            "tsx".to_string(),
            "src/index.ts".to_string(),
        ],
        seccomp_profile: "default".to_string(),
        resources: limits(),
        oidc: Some(oidc()),
    }
}

fn config() -> WorkerRuntimeDockerConfig {
    WorkerRuntimeDockerConfig {
        workflow: Some(workflow_config()),
        recording: Some(recording_config()),
    }
}

fn secret(value: &str) -> SecretValue {
    SecretValue::new(value).unwrap()
}

fn operation(operation: RuntimeOperation) -> RuntimeOperationRequest {
    RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::now_v7(),
        idempotency_key: IdempotencyKey::new(format!("test:{}", Uuid::now_v7())).unwrap(),
        operation,
    }
}

fn lifecycle(
    kind: RuntimeOperationKind,
    resource_id: Uuid,
    action: ContainerLifecycleAction,
) -> RuntimeOperationRequest {
    operation(RuntimeOperation::ContainerLifecycle(
        ContainerLifecycleRequest {
            operation_kind: kind,
            resource_id,
            action,
        },
    ))
}

fn environment(body: &ContainerCreateBody) -> BTreeMap<String, String> {
    body.env
        .as_ref()
        .unwrap()
        .iter()
        .map(|entry| {
            let (key, value) = entry.split_once('=').unwrap();
            (key.to_string(), value.to_string())
        })
        .collect()
}

fn delivered_secrets(call: &BackendCall) -> BTreeMap<String, String> {
    let BackendCall::SendStdin { contents, .. } = call else {
        panic!("backend call must deliver worker secrets");
    };
    serde_json::from_slice(contents).unwrap()
}

#[tokio::test]
async fn workflow_launch_materializes_only_fixed_policy_fields() {
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter = WorkerRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let run_id = Uuid::now_v7();
    let session_id = Uuid::now_v7();
    let task_id = Uuid::now_v7();

    let result = adapter
        .execute(&operation(RuntimeOperation::LaunchWorkflow(
            WorkflowWorkerLaunchRequest {
                workflow_run_id: run_id,
                session_id,
                automation_task_id: task_id,
                credentials: WorkflowWorkerCredentials {
                    session_automation_access_token: secret("automation-secret"),
                    gateway_bearer_token: Some(secret("gateway-secret")),
                },
            },
        )))
        .await
        .unwrap();

    assert_eq!(result, RuntimeOperationResult::Accepted);
    let calls = backend.calls.lock().unwrap();
    assert_eq!(calls.len(), 4);
    assert!(matches!(&calls[0], BackendCall::Remove));
    let BackendCall::Create { name, body } = &calls[1] else {
        panic!("second backend call must create the worker");
    };
    assert_eq!(name, &format!("bpane-workflow-{}", run_id.simple()));
    assert_eq!(body.image, Some(workflow_config().image));
    assert_eq!(body.cmd, Some(workflow_config().command));
    let host = body.host_config.as_ref().unwrap();
    assert_eq!(host.network_mode.as_deref(), Some("bpane-internal"));
    assert_eq!(host.auto_remove, Some(false));
    assert_eq!(host.privileged, Some(false));
    assert_eq!(
        host.security_opt.as_deref(),
        Some(["no-new-privileges:true".to_string()].as_slice())
    );
    assert!(host.binds.is_none());
    assert!(host.mounts.is_none());
    assert!(host.devices.is_none());
    assert!(host.cap_add.is_none());
    assert!(host.tmpfs.is_none());
    assert_eq!(host.memory, Some(limits().memory_bytes as i64));
    assert_eq!(host.pids_limit, Some(i64::from(limits().pids)));
    assert_eq!(
        host.log_config.as_ref().unwrap().config.as_ref().unwrap(),
        &std::collections::HashMap::from([
            ("max-size".to_string(), "262144b".to_string()),
            ("max-file".to_string(), "1".to_string()),
            ("compress".to_string(), "false".to_string()),
        ])
    );
    let env = environment(body);
    assert_eq!(env.get("BPANE_WORKFLOW_RUN_ID"), Some(&run_id.to_string()));
    assert_eq!(
        env.get("BPANE_WORKER_SECRETS_STDIN").map(String::as_str),
        Some("true")
    );
    assert!(!env.contains_key("BPANE_SESSION_AUTOMATION_ACCESS_TOKEN"));
    assert!(!env.contains_key("BPANE_WORKFLOW_BEARER_TOKEN"));
    assert!(!env.contains_key("BPANE_GATEWAY_OIDC_CLIENT_SECRET"));
    assert!(!env.contains_key("BPANE_GATEWAY_OIDC_SCOPES"));
    assert!(matches!(&calls[2], BackendCall::Start(value) if value == name));
    assert_eq!(body.attach_stdin, Some(true));
    assert_eq!(body.open_stdin, Some(true));
    assert_eq!(body.stdin_once, Some(true));
    let BackendCall::SendStdin {
        name: target_name, ..
    } = &calls[3]
    else {
        panic!("fourth backend call must deliver worker secrets");
    };
    assert_eq!(target_name, name);
    assert_eq!(
        delivered_secrets(&calls[3]),
        BTreeMap::from([
            (
                "BPANE_GATEWAY_OIDC_CLIENT_SECRET".to_string(),
                "oidc-secret".to_string(),
            ),
            (
                "BPANE_SESSION_AUTOMATION_ACCESS_TOKEN".to_string(),
                "automation-secret".to_string(),
            ),
            (
                "BPANE_WORKFLOW_BEARER_TOKEN".to_string(),
                "gateway-secret".to_string(),
            ),
        ])
    );
}

#[tokio::test]
async fn recording_launch_allows_only_the_fixed_artifact_volume() {
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter = WorkerRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let session_id = Uuid::now_v7();
    let recording_id = Uuid::now_v7();

    let result = adapter
        .execute(&operation(RuntimeOperation::LaunchRecording(
            RecordingWorkerLaunchRequest {
                session_id,
                recording_id,
                credentials: RecordingWorkerCredentials {
                    connect_ticket: secret("connect-secret"),
                    session_automation_access_token: secret("automation-secret"),
                    recording_worker_access_token: secret("worker-secret"),
                    gateway_bearer_token: Some(secret("gateway-secret")),
                },
            },
        )))
        .await
        .unwrap();

    assert_eq!(result, RuntimeOperationResult::Accepted);
    let calls = backend.calls.lock().unwrap();
    let BackendCall::Create { name, body } = &calls[1] else {
        panic!("second backend call must create the worker");
    };
    assert_eq!(name, &format!("bpane-recording-{}", recording_id.simple()));
    let host = body.host_config.as_ref().unwrap();
    assert!(host.binds.is_none());
    let mounts = host.mounts.as_ref().unwrap();
    assert_eq!(mounts.len(), 1);
    assert_eq!(mounts[0].source.as_deref(), Some("bpane-recordings"));
    assert_eq!(mounts[0].target.as_deref(), Some("/tmp/bpane-recordings"));
    assert_eq!(mounts[0].read_only, Some(false));
    let env = environment(body);
    assert_eq!(
        env.get("BPANE_RECORDING_SESSION_ID"),
        Some(&session_id.to_string())
    );
    assert_eq!(
        env.get("BPANE_RECORDING_ID"),
        Some(&recording_id.to_string())
    );
    assert!(!env.contains_key("BPANE_RECORDING_CONNECT_TICKET"));
    assert!(!env.contains_key("BPANE_SESSION_AUTOMATION_ACCESS_TOKEN"));
    assert!(!env.contains_key("BPANE_RECORDING_WORKER_ACCESS_TOKEN"));
    assert!(!env.contains_key("BPANE_RECORDING_BEARER_TOKEN"));
    assert!(!env.contains_key("BPANE_GATEWAY_OIDC_CLIENT_SECRET"));
    assert_eq!(
        env.get("BPANE_RECORDING_CONNECT_GATEWAY_URL")
            .map(String::as_str),
        Some("https://gateway:4433")
    );
    assert_eq!(
        delivered_secrets(&calls[3]),
        BTreeMap::from([
            (
                "BPANE_GATEWAY_OIDC_CLIENT_SECRET".to_string(),
                "oidc-secret".to_string(),
            ),
            (
                "BPANE_RECORDING_BEARER_TOKEN".to_string(),
                "gateway-secret".to_string(),
            ),
            (
                "BPANE_RECORDING_CONNECT_TICKET".to_string(),
                "connect-secret".to_string(),
            ),
            (
                "BPANE_RECORDING_WORKER_ACCESS_TOKEN".to_string(),
                "worker-secret".to_string(),
            ),
            (
                "BPANE_SESSION_AUTOMATION_ACCESS_TOKEN".to_string(),
                "automation-secret".to_string(),
            ),
        ])
    );
}

#[tokio::test]
async fn stdin_delivery_failure_removes_the_started_worker_and_returns_a_sanitized_error() {
    let backend = Arc::new(FakeDockerBackend::default());
    backend.fail_next("send_stdin", DockerBackendError::Failed);
    let adapter = WorkerRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let error = adapter
        .execute(&operation(RuntimeOperation::LaunchWorkflow(
            WorkflowWorkerLaunchRequest {
                workflow_run_id: Uuid::now_v7(),
                session_id: Uuid::now_v7(),
                automation_task_id: Uuid::now_v7(),
                credentials: WorkflowWorkerCredentials {
                    session_automation_access_token: secret("do-not-leak"),
                    gateway_bearer_token: None,
                },
            },
        )))
        .await
        .unwrap_err();

    assert_eq!(error.code, ExecutionErrorCode::AdapterFailed);
    assert!(!format!("{error:?}").contains("do-not-leak"));
    assert!(matches!(
        backend.calls.lock().unwrap().last(),
        Some(BackendCall::Remove)
    ));
}

#[tokio::test]
async fn lifecycle_reports_detached_worker_state_and_idempotent_absence() {
    let backend = Arc::new(FakeDockerBackend::with_state(DockerContainerState::Running));
    let adapter = WorkerRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let run_id = Uuid::now_v7();

    let running = adapter
        .execute(&lifecycle(
            RuntimeOperationKind::WorkflowWorker,
            run_id,
            ContainerLifecycleAction::Inspect,
        ))
        .await
        .unwrap();
    assert_eq!(
        running,
        RuntimeOperationResult::WorkerState {
            execution_state: WorkerExecutionState::Running,
            exit_code: None,
        }
    );

    backend.set_state(DockerContainerState::Exited {
        exit_code: Some(17),
    });
    let exited = adapter
        .execute(&lifecycle(
            RuntimeOperationKind::WorkflowWorker,
            run_id,
            ContainerLifecycleAction::Inspect,
        ))
        .await
        .unwrap();
    assert_eq!(
        exited,
        RuntimeOperationResult::WorkerState {
            execution_state: WorkerExecutionState::Exited,
            exit_code: Some(17),
        }
    );

    assert!(matches!(
        adapter
            .execute(&lifecycle(
                RuntimeOperationKind::WorkflowWorker,
                run_id,
                ContainerLifecycleAction::Remove
            ))
            .await
            .unwrap(),
        RuntimeOperationResult::Completed { .. }
    ));
    assert_eq!(
        adapter
            .execute(&lifecycle(
                RuntimeOperationKind::WorkflowWorker,
                run_id,
                ContainerLifecycleAction::Remove
            ))
            .await
            .unwrap(),
        RuntimeOperationResult::Absent
    );
}

#[tokio::test]
async fn readiness_and_backend_failures_are_sanitized() {
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter = WorkerRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    adapter.check_readiness().await.unwrap();
    assert!(matches!(
        backend.calls.lock().unwrap().as_slice(),
        [BackendCall::Ping]
    ));

    backend.fail_next("ping", DockerBackendError::Failed);
    assert_eq!(
        adapter.check_readiness().await.unwrap_err().code,
        ExecutionErrorCode::AdapterFailed
    );
    backend.fail_next("create", DockerBackendError::Failed);
    let error = adapter
        .execute(&operation(RuntimeOperation::LaunchWorkflow(
            WorkflowWorkerLaunchRequest {
                workflow_run_id: Uuid::now_v7(),
                session_id: Uuid::now_v7(),
                automation_task_id: Uuid::now_v7(),
                credentials: WorkflowWorkerCredentials {
                    session_automation_access_token: secret("do-not-leak"),
                    gateway_bearer_token: None,
                },
            },
        )))
        .await
        .unwrap_err();
    assert_eq!(error.code, ExecutionErrorCode::AdapterFailed);
    assert!(!format!("{error:?}").contains("do-not-leak"));
}

#[test]
fn configuration_rejects_mutable_images_and_redacts_oidc_secrets() {
    let backend = Arc::new(FakeDockerBackend::default());
    let mut invalid = config();
    invalid.workflow.as_mut().unwrap().image = "workflow-worker:latest".to_string();
    assert_eq!(
        WorkerRuntimeDockerAdapter::with_backend(invalid, backend.clone())
            .unwrap_err()
            .code,
        ExecutionErrorCode::AdapterFailed
    );

    let adapter = WorkerRuntimeDockerAdapter::with_backend(config(), backend).unwrap();
    let debug = format!("{adapter:?}");
    assert!(debug.contains("[REDACTED]"));
    assert!(!debug.contains("oidc-secret"));
}

#[tokio::test]
async fn unsupported_operation_families_fail_closed() {
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter = WorkerRuntimeDockerAdapter::with_backend(config(), backend).unwrap();
    let error = adapter
        .execute(&lifecycle(
            RuntimeOperationKind::BrowserRuntime,
            Uuid::now_v7(),
            ContainerLifecycleAction::Inspect,
        ))
        .await
        .unwrap_err();
    assert_eq!(error.code, ExecutionErrorCode::AdapterUnavailable);
}
