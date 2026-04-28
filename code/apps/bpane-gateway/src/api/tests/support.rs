use super::*;

#[derive(Default)]
struct TestCredentialProviderBackend {
    secrets: Mutex<HashMap<String, Value>>,
}

#[async_trait]
impl CredentialProviderBackend for TestCredentialProviderBackend {
    async fn store_secret(
        &self,
        request: StoreCredentialSecretRequest,
    ) -> Result<StoredCredentialSecret, CredentialProviderError> {
        let external_ref = request
            .external_ref
            .unwrap_or_else(|| format!("test/{}", request.binding_id));
        self.secrets
            .lock()
            .await
            .insert(external_ref.clone(), request.payload);
        Ok(StoredCredentialSecret { external_ref })
    }

    async fn resolve_secret(
        &self,
        external_ref: &str,
    ) -> Result<ResolvedCredentialSecret, CredentialProviderError> {
        let payload = self
            .secrets
            .lock()
            .await
            .get(external_ref)
            .cloned()
            .ok_or_else(|| {
                CredentialProviderError::Backend(format!(
                    "test credential secret {external_ref} not found"
                ))
            })?;
        Ok(ResolvedCredentialSecret { payload })
    }
}

pub(crate) fn test_router_with_state() -> (Router, String, Arc<ApiState>) {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        session_store: SessionStore::in_memory(),
        session_manager: Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: "/tmp/test.sock".to_string(),
                cdp_endpoint: Some("http://host:9223".to_string()),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle: Arc::new(WorkflowLifecycleManager::disabled()),
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state.clone()), token, state)
}

pub(crate) fn test_router() -> (Router, String) {
    let (router, token, _) = test_router_with_state();
    (router, token)
}

pub(crate) fn test_router_with_workflow_lifecycle(
    config: WorkflowWorkerConfig,
) -> (Router, String, Arc<ApiState>) {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let automation_access_token_manager = Arc::new(SessionAutomationAccessTokenManager::new(
        vec![6; 32],
        Duration::from_secs(300),
    ));
    let session_store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "workflow_test_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 4,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let session_manager = Arc::new(
        SessionManager::new(SessionManagerConfig::StaticSingle {
            agent_socket_path: "/tmp/test.sock".to_string(),
            cdp_endpoint: Some("http://host:9223".to_string()),
            idle_timeout: Duration::from_secs(300),
        })
        .unwrap(),
    );
    let registry = Arc::new(SessionRegistry::new(10, false));
    let workflow_lifecycle = Arc::new(
        WorkflowLifecycleManager::new(
            Some(config),
            auth_validator.clone(),
            automation_access_token_manager.clone(),
            session_store.clone(),
            session_manager.clone(),
            registry.clone(),
        )
        .expect("workflow lifecycle test config should be valid"),
    );
    let state = Arc::new(ApiState {
        registry,
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager,
        session_store,
        session_manager,
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle,
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state.clone()), token, state)
}

pub(crate) fn create_sleep_workflow_worker_script(
    dir: &tempfile::TempDir,
    capture_file: &std::path::Path,
    sleep_seconds: f32,
) -> std::path::PathBuf {
    let script_path = dir.path().join("workflow-worker-test.sh");
    fs::write(
        &script_path,
        format!(
            r#"#!/bin/sh
printf '%s\n' "$@" >> "{}"
sleep {}
"#,
            capture_file.display(),
            sleep_seconds,
        ),
    )
    .unwrap();
    let mut permissions = fs::metadata(&script_path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&script_path, permissions).unwrap();
    script_path
}

pub(crate) fn test_artifact_store() -> Arc<RecordingArtifactStore> {
    let root = std::env::temp_dir().join(format!("bpane-artifacts-test-{}", uuid::Uuid::now_v7()));
    Arc::new(RecordingArtifactStore::local_fs(root))
}

pub(crate) fn test_workspace_file_store() -> Arc<WorkspaceFileStore> {
    let root = std::env::temp_dir().join(format!(
        "bpane-workspace-files-test-{}",
        uuid::Uuid::now_v7()
    ));
    Arc::new(WorkspaceFileStore::local_fs(root))
}

pub(crate) fn test_credential_provider() -> Arc<CredentialProvider> {
    Arc::new(CredentialProvider::new(Arc::new(
        TestCredentialProviderBackend::default(),
    )))
}

pub(crate) fn test_workflow_source_resolver() -> Arc<WorkflowSourceResolver> {
    Arc::new(WorkflowSourceResolver::new(std::path::PathBuf::from("git")))
}

#[derive(Debug, Clone)]
pub(crate) struct CapturedWebhookRequest {
    pub(crate) headers: HashMap<String, String>,
    pub(crate) body: Value,
}

#[derive(Clone, Default)]
struct TestWebhookReceiverState {
    requests: Arc<Mutex<Vec<CapturedWebhookRequest>>>,
    statuses: Arc<Mutex<Vec<StatusCode>>>,
}

pub(crate) struct TestWebhookReceiver {
    pub(crate) url: String,
    state: TestWebhookReceiverState,
    shutdown: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}

impl TestWebhookReceiver {
    pub(crate) async fn start(statuses: Vec<StatusCode>) -> Self {
        let state = TestWebhookReceiverState {
            requests: Arc::new(Mutex::new(Vec::new())),
            statuses: Arc::new(Mutex::new(statuses)),
        };
        let app = axum::Router::new().route(
            "/events",
            axum::routing::post({
                let state = state.clone();
                move |headers: axum::http::HeaderMap, body: axum::body::Bytes| {
                    let state = state.clone();
                    async move {
                        let body = serde_json::from_slice::<Value>(&body).unwrap();
                        let mut captured_headers = HashMap::new();
                        for name in [
                            "x-bpane-event-id",
                            "x-bpane-event-type",
                            "x-bpane-delivery-id",
                            "x-bpane-subscription-id",
                            "x-bpane-signature-timestamp",
                            "x-bpane-signature-v1",
                        ] {
                            if let Some(value) =
                                headers.get(name).and_then(|value| value.to_str().ok())
                            {
                                captured_headers.insert(name.to_string(), value.to_string());
                            }
                        }
                        state.requests.lock().await.push(CapturedWebhookRequest {
                            headers: captured_headers,
                            body,
                        });
                        let status = {
                            let mut statuses = state.statuses.lock().await;
                            if statuses.is_empty() {
                                StatusCode::OK
                            } else {
                                statuses.remove(0)
                            }
                        };
                        (status, "ok")
                    }
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test webhook receiver");
        let address = listener.local_addr().expect("receiver addr");
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .await
                .expect("run webhook receiver");
        });
        Self {
            url: format!("http://{address}/events"),
            state,
            shutdown: Some(shutdown_tx),
            task,
        }
    }

    pub(crate) async fn requests(&self) -> Vec<CapturedWebhookRequest> {
        self.state.requests.lock().await.clone()
    }
}

impl Drop for TestWebhookReceiver {
    fn drop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
        self.task.abort();
    }
}

pub(crate) struct TestAgentServer {
    socket_path: std::path::PathBuf,
    accept_task: tokio::task::JoinHandle<()>,
}

impl TestAgentServer {
    async fn start() -> Self {
        let socket_path = std::path::PathBuf::from(format!(
            "/tmp/bpane-agent-{}.sock",
            uuid::Uuid::now_v7().simple()
        ));
        let _ = std::fs::remove_file(&socket_path);
        let listener = tokio::net::UnixListener::bind(&socket_path).unwrap();
        let accept_task = tokio::spawn(async move {
            let mut connections = Vec::new();
            while let Ok((stream, _)) = listener.accept().await {
                connections.push(stream);
            }
        });

        Self {
            socket_path,
            accept_task,
        }
    }

    fn socket_path(&self) -> String {
        self.socket_path.to_string_lossy().into_owned()
    }
}

impl Drop for TestAgentServer {
    fn drop(&mut self) {
        self.accept_task.abort();
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

pub(crate) async fn test_router_with_live_agent() -> (Router, String, TestAgentServer) {
    let agent_server = TestAgentServer::start().await;
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        session_store: SessionStore::in_memory(),
        session_manager: Arc::new(
            SessionManager::new(SessionManagerConfig::StaticSingle {
                agent_socket_path: agent_server.socket_path(),
                cdp_endpoint: Some("http://host:9223".to_string()),
                idle_timeout: Duration::from_secs(300),
            })
            .unwrap(),
        ),
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle: Arc::new(WorkflowLifecycleManager::disabled()),
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state), token, agent_server)
}

pub(crate) async fn test_router_with_docker_pool() -> (Router, String) {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let session_manager = Arc::new(
        SessionManager::new(SessionManagerConfig::DockerPool(
            crate::session_manager::SessionManagerDockerConfig {
                docker_bin: "docker".to_string(),
                image: "deploy-host".to_string(),
                network: "deploy_bpane-internal".to_string(),
                shared_run_volume: "deploy_agent-socket".to_string(),
                container_name_prefix: "bpane-runtime".to_string(),
                socket_root: "/run/bpane/sessions".to_string(),
                cdp_proxy_port: 9223,
                shm_size: "128m".to_string(),
                start_timeout: Duration::from_secs(30),
                idle_timeout: Duration::from_secs(300),
                max_active_runtimes: 2,
                max_starting_runtimes: 1,
                seccomp_unconfined: true,
                env_file: None,
            },
        ))
        .unwrap(),
    );
    let session_store = SessionStore::in_memory_with_config(session_manager.profile().clone());
    session_manager
        .attach_session_store(session_store.clone())
        .await;
    let state = Arc::new(ApiState {
        registry: Arc::new(SessionRegistry::new(10, false)),
        auth_validator,
        connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
            vec![5; 32],
            Duration::from_secs(300),
        )),
        automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
            vec![6; 32],
            Duration::from_secs(300),
        )),
        session_store,
        session_manager,
        credential_provider: Some(test_credential_provider()),
        recording_artifact_store: test_artifact_store(),
        workspace_file_store: test_workspace_file_store(),
        workflow_source_resolver: test_workflow_source_resolver(),
        recording_observability: Arc::new(RecordingObservability::default()),
        recording_lifecycle: Arc::new(RecordingLifecycleManager::disabled()),
        workflow_lifecycle: Arc::new(WorkflowLifecycleManager::disabled()),
        workflow_observability: Arc::new(WorkflowObservability::default()),
        workflow_log_retention: None,
        workflow_output_retention: None,
        idle_stop_timeout: Duration::from_secs(300),
        public_gateway_url: "https://localhost:4433".to_string(),
        default_owner_mode: SessionOwnerMode::Collaborative,
    });
    (build_api_router(state), token)
}

pub(crate) fn bearer(token: &str) -> String {
    format!("Bearer {token}")
}

pub(crate) async fn response_json(response: axum::response::Response) -> Value {
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

pub(crate) async fn response_bytes(response: axum::response::Response) -> Vec<u8> {
    to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec()
}

pub(crate) fn git(args: &[&str], cwd: &std::path::Path) {
    let output = StdCommand::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {:?} failed: {}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(crate) fn git_head(cwd: &std::path::Path) -> String {
    let output = StdCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(output.status.success());
    String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_ascii_lowercase()
}
