use crate::session_control::SessionStore;
use crate::session_registry::SessionRegistry;

use super::super::*;
use super::agent::TestAgentServer;

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

fn base_api_state(auth_validator: Arc<AuthValidator>) -> Arc<ApiState> {
    Arc::new(ApiState {
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
    })
}

pub(crate) fn test_router_with_state() -> (Router, String, Arc<ApiState>) {
    let auth_validator = Arc::new(AuthValidator::from_hmac_secret(vec![7; 32]));
    let token = auth_validator
        .generate_token()
        .expect("hmac auth validator should generate dev token");
    let state = base_api_state(auth_validator);
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
