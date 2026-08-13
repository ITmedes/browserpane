use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{Method, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use axum::Router;
use bollard::models::ContainerCreateBody;
use bpane_runtime_contract::{
    BrokerApiVersion, BrowserEgressObservationMode, BrowserEgressSelection, BrowserGeolocation,
    BrowserNetworkIdentity, BrowserProxySelection, BrowserRuntimeFeatures,
    BrowserRuntimeLaunchRequest, BrowserSessionDataSource, ContainerLifecycleAction,
    ContainerLifecycleRequest, IdempotencyKey, ResourceLimits, RuntimeOperation,
    RuntimeOperationKind, VolumeLifecycleAction, VolumeLifecycleRequest,
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
    Inspect(String),
    Stop(String),
    Remove(String),
}

#[derive(Default)]
struct FakeDockerBackend {
    calls: Mutex<Vec<BackendCall>>,
    failures: Mutex<VecDeque<(&'static str, DockerBackendError)>>,
    exists: AtomicBool,
}

impl FakeDockerBackend {
    fn existing() -> Self {
        Self {
            exists: AtomicBool::new(true),
            ..Self::default()
        }
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
        self.exists.store(true, Ordering::SeqCst);
        Ok(())
    }

    async fn start(&self, name: &str) -> Result<(), DockerBackendError> {
        self.calls
            .lock()
            .unwrap()
            .push(BackendCall::Start(name.to_string()));
        self.failure("start").map_or(Ok(()), Err)
    }

    async fn inspect(&self, name: &str) -> Result<(), DockerBackendError> {
        self.calls
            .lock()
            .unwrap()
            .push(BackendCall::Inspect(name.to_string()));
        if let Some(error) = self.failure("inspect") {
            return Err(error);
        }
        if self.exists.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err(DockerBackendError::NotFound)
        }
    }

    async fn stop(&self, name: &str) -> Result<(), DockerBackendError> {
        self.calls
            .lock()
            .unwrap()
            .push(BackendCall::Stop(name.to_string()));
        if let Some(error) = self.failure("stop") {
            return Err(error);
        }
        if self.exists.swap(false, Ordering::SeqCst) {
            Ok(())
        } else {
            Err(DockerBackendError::NotFound)
        }
    }

    async fn remove(&self, name: &str) -> Result<(), DockerBackendError> {
        self.calls
            .lock()
            .unwrap()
            .push(BackendCall::Remove(name.to_string()));
        if let Some(error) = self.failure("remove") {
            return Err(error);
        }
        if self.exists.swap(false, Ordering::SeqCst) {
            Ok(())
        } else {
            Err(DockerBackendError::NotFound)
        }
    }
}

fn config() -> BrowserRuntimeDockerConfig {
    BrowserRuntimeDockerConfig {
        image: format!(
            "registry.example/browserpane/host@sha256:{}",
            "a".repeat(64)
        ),
        network: "bpane-internal".to_string(),
        socket_volume: "bpane-agent-socket".to_string(),
        session_data_volume_prefix: "bpane-session-data".to_string(),
        browser_context_volume_prefix: "bpane-browser-context".to_string(),
        container_name_prefix: "bpane-runtime".to_string(),
        socket_mount_root: "/run/bpane".to_string(),
        socket_path_root: "/run/bpane/sessions".to_string(),
        session_data_root: "/run/bpane/session".to_string(),
        command: vec!["/usr/local/bin/start-host.sh".to_string()],
        seccomp_profile: "default".to_string(),
        resources: ResourceLimits {
            memory_bytes: 4 * 1024 * 1024 * 1024,
            cpu_millis: 4_000,
            pids: 1_024,
            shm_bytes: 512 * 1024 * 1024,
            timeout_secs: 120,
            output_limit_bytes: 65_536,
        },
        extensions: BTreeMap::new(),
        base_environment: BTreeMap::new(),
    }
}

fn operation(operation: RuntimeOperation) -> RuntimeOperationRequest {
    RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::now_v7(),
        idempotency_key: IdempotencyKey::new(format!("test:{}", Uuid::now_v7())).unwrap(),
        operation,
    }
}

fn lifecycle(resource_id: Uuid, action: ContainerLifecycleAction) -> RuntimeOperationRequest {
    operation(RuntimeOperation::ContainerLifecycle(
        ContainerLifecycleRequest {
            operation_kind: RuntimeOperationKind::BrowserRuntime,
            resource_id,
            action,
        },
    ))
}

#[tokio::test]
async fn readiness_pings_the_docker_dependency_and_sanitizes_failures() {
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter = BrowserRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();

    adapter.check_readiness().await.unwrap();
    assert!(matches!(
        backend.calls.lock().unwrap().as_slice(),
        [BackendCall::Ping]
    ));

    backend.fail_next("ping", DockerBackendError::Failed);
    let error = adapter.check_readiness().await.unwrap_err();
    assert_eq!(error.code, ExecutionErrorCode::AdapterFailed);
}

#[tokio::test]
async fn launch_materializes_only_broker_owned_container_fields() {
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter = BrowserRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let session_id = Uuid::now_v7();

    let result = adapter
        .execute(&operation(RuntimeOperation::LaunchBrowser(
            BrowserRuntimeLaunchRequest {
                session_id,
                browser_context_id: None,
                features: Default::default(),
            },
        )))
        .await
        .unwrap();

    assert_eq!(result, RuntimeOperationResult::Accepted);
    let calls = backend.calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert!(matches!(&calls[0], BackendCall::Remove(_)));
    let BackendCall::Create { name, body } = &calls[1] else {
        panic!("second backend call must create the container");
    };
    let expected_name = format!("bpane-runtime-{}", session_id.simple());
    assert_eq!(name, &expected_name);
    assert_eq!(body.image, Some(config().image));
    assert_eq!(body.cmd, Some(config().command));
    assert_eq!(
        body.labels.as_ref().unwrap(),
        &std::collections::HashMap::from([
            (
                "browserpane.runtime.operation".to_string(),
                "browser_runtime".to_string(),
            ),
            (
                "browserpane.runtime.resource_id".to_string(),
                session_id.to_string(),
            ),
        ])
    );
    let host = body.host_config.as_ref().unwrap();
    assert_eq!(host.network_mode.as_deref(), Some("bpane-internal"));
    assert_eq!(host.privileged, Some(false));
    assert_eq!(host.readonly_rootfs, Some(false));
    assert_eq!(
        host.security_opt.as_deref(),
        Some(["no-new-privileges:true".to_string()].as_slice())
    );
    assert!(host.binds.is_none());
    assert!(host.devices.is_none());
    assert!(host.cap_add.is_none());
    let mounts = host.mounts.as_ref().unwrap();
    assert_eq!(mounts.len(), 2);
    assert!(mounts
        .iter()
        .all(|mount| mount.typ == Some(bollard::models::MountType::VOLUME)));
    assert!(mounts.iter().any(|mount| {
        mount.source.as_deref() == Some("bpane-agent-socket")
            && mount.target.as_deref() == Some("/run/bpane")
    }));
    assert!(mounts.iter().any(|mount| {
        mount.source.as_deref()
            == Some(format!("bpane-session-data-{}", session_id.simple()).as_str())
            && mount.target.as_deref() == Some("/run/bpane/session")
    }));
    assert!(body.env.as_ref().unwrap().contains(&format!(
        "BPANE_SOCKET_PATH=/run/bpane/sessions/{session_id}.sock"
    )));
    assert!(matches!(&calls[2], BackendCall::Start(name) if name == &expected_name));
}

#[tokio::test]
async fn reusable_context_adds_only_the_owned_profile_volume() {
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter = BrowserRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let session_id = Uuid::now_v7();
    let context_id = Uuid::now_v7();

    adapter
        .execute(&operation(RuntimeOperation::LaunchBrowser(
            BrowserRuntimeLaunchRequest {
                session_id,
                browser_context_id: Some(context_id),
                features: Default::default(),
            },
        )))
        .await
        .unwrap();

    let calls = backend.calls.lock().unwrap();
    let BackendCall::Create { body, .. } = &calls[1] else {
        panic!("second backend call must create the container");
    };
    let mounts = body.host_config.as_ref().unwrap().mounts.as_ref().unwrap();
    assert_eq!(mounts.len(), 3);
    assert!(mounts.iter().any(|mount| {
        mount.source.as_deref()
            == Some(format!("bpane-browser-context-{}", context_id.simple()).as_str())
            && mount.target.as_deref() == Some("/run/bpane/session/chromium")
    }));
}

#[tokio::test]
async fn launch_materializes_identity_tls_egress_and_approved_extensions() {
    let extension_id = Uuid::now_v7();
    let mut runtime_config = config();
    runtime_config.extensions.insert(
        extension_id,
        BrowserRuntimeExtensionConfig {
            extension_version_id: extension_id,
            install_path: "/home/bpane/extensions/approved".to_string(),
        },
    );
    runtime_config
        .base_environment
        .insert("BPANE_FPS".to_string(), "60".to_string());
    runtime_config.base_environment.insert(
        "BPANE_URL".to_string(),
        "https://runtime-default.example".to_string(),
    );
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter =
        BrowserRuntimeDockerAdapter::with_backend(runtime_config, backend.clone()).unwrap();
    let session_id = Uuid::now_v7();
    let profile_id = Uuid::now_v7();

    adapter
        .execute(&operation(RuntimeOperation::LaunchBrowser(
            BrowserRuntimeLaunchRequest {
                session_id,
                browser_context_id: None,
                features: BrowserRuntimeFeatures {
                    network_identity: BrowserNetworkIdentity {
                        locale: Some("de-DE".to_string()),
                        languages: vec!["de-DE".to_string(), "en-US".to_string()],
                        timezone: Some("Europe/Berlin".to_string()),
                        geolocation: Some(BrowserGeolocation {
                            latitude_e7: 525_200_000,
                            longitude_e7: 134_050_000,
                            accuracy_mm: Some(25_000),
                        }),
                        user_agent: Some("BrowserPane test agent".to_string()),
                        browser_identity: Some("regulated-pilot".to_string()),
                    },
                    egress: Some(BrowserEgressSelection {
                        profile_id,
                        proxy: Some(BrowserProxySelection {
                            url: "http://proxy.internal:3128".to_string(),
                            authentication: Some(BrowserSessionDataSource::SessionData),
                        }),
                        bypass_rules: vec!["localhost".to_string(), "*.internal".to_string()],
                        observation_mode: BrowserEgressObservationMode::TlsIntercept,
                        custom_ca: Some(BrowserSessionDataSource::SessionData),
                        sensitive_log_sink_configured: true,
                    }),
                    extension_version_ids: vec![extension_id],
                    session_file_bindings: true,
                },
            },
        )))
        .await
        .unwrap();

    let calls = backend.calls.lock().unwrap();
    let BackendCall::Create { body, .. } = &calls[1] else {
        panic!("second backend call must create the container");
    };
    let environment = body.env.as_ref().unwrap();
    for expected in [
        "LANG=de_DE.UTF-8",
        "LC_ALL=de_DE.UTF-8",
        "LANGUAGE=de-DE:en-US",
        "BPANE_CHROMIUM_LANG=de-DE",
        "BPANE_CHROMIUM_ACCEPT_LANG=de-DE,en-US",
        "TZ=Europe/Berlin",
        "BPANE_CHROMIUM_USER_AGENT=BrowserPane test agent",
        "BPANE_BROWSER_IDENTITY=regulated-pilot",
        "BPANE_CHROMIUM_PROXY_SERVER=http://proxy.internal:3128",
        "BPANE_CHROMIUM_PROXY_AUTH_FILE=/run/bpane/session/egress/proxy-auth.json",
        "BPANE_CHROMIUM_PROXY_BYPASS_LIST=localhost;*.internal",
        "BPANE_CHROMIUM_TRUSTED_CA_BUNDLE=/run/bpane/session/egress/custom-ca.pem",
        "BPANE_CHROMIUM_TRUSTED_CA_NAME=BrowserPane Egress Interception CA",
        "BPANE_EXTENSION_DIRS=/home/bpane/extensions/approved",
        "BPANE_SESSION_FILE_BINDINGS_MANIFEST=/run/bpane/session/session-file-bindings.json",
        "BPANE_FPS=60",
        "BPANE_URL=about:blank",
    ] {
        assert!(
            environment.iter().any(|value| value == expected),
            "{expected}"
        );
    }
    assert_eq!(
        environment
            .iter()
            .filter(|value| value.starts_with("BPANE_URL="))
            .count(),
        1,
        "feature-derived values must override trusted defaults without duplicates"
    );
    let geolocation = environment
        .iter()
        .find_map(|value| value.strip_prefix("BPANE_SESSION_GEOLOCATION="))
        .unwrap();
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(geolocation).unwrap(),
        serde_json::json!({
            "latitude": 52.52,
            "longitude": 13.405,
            "accuracy_meters": 25.0
        })
    );
    assert!(environment
        .iter()
        .all(|value| !value.contains("password") && !value.contains("secret")));
    let labels = body.labels.as_ref().unwrap();
    assert_eq!(
        labels.get("browserpane.egress_profile_id"),
        Some(&profile_id.to_string())
    );
    assert_eq!(
        labels.get("browserpane.egress_observation_mode"),
        Some(&"tls_intercept".to_string())
    );
    assert_eq!(
        labels.get("browserpane.egress_proxy_auth_configured"),
        Some(&"true".to_string())
    );
}

#[tokio::test]
async fn metadata_only_egress_does_not_materialize_tls_or_auth_prerequisites() {
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter = BrowserRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    adapter
        .execute(&operation(RuntimeOperation::LaunchBrowser(
            BrowserRuntimeLaunchRequest {
                session_id: Uuid::now_v7(),
                browser_context_id: None,
                features: BrowserRuntimeFeatures {
                    egress: Some(BrowserEgressSelection {
                        profile_id: Uuid::now_v7(),
                        proxy: Some(BrowserProxySelection {
                            url: "https://proxy.internal:8443".to_string(),
                            authentication: None,
                        }),
                        bypass_rules: Vec::new(),
                        observation_mode: BrowserEgressObservationMode::MetadataOnly,
                        custom_ca: None,
                        sensitive_log_sink_configured: false,
                    }),
                    ..Default::default()
                },
            },
        )))
        .await
        .unwrap();

    let calls = backend.calls.lock().unwrap();
    let BackendCall::Create { body, .. } = &calls[1] else {
        panic!("second backend call must create the container");
    };
    let environment = body.env.as_ref().unwrap();
    assert!(environment
        .iter()
        .any(|value| value == "BPANE_CHROMIUM_PROXY_SERVER=https://proxy.internal:8443"));
    assert!(environment.iter().all(|value| {
        !value.starts_with("BPANE_CHROMIUM_PROXY_AUTH_FILE=")
            && !value.starts_with("BPANE_CHROMIUM_TRUSTED_CA_BUNDLE=")
    }));
}

#[tokio::test]
async fn unknown_extension_version_is_denied_before_docker_dispatch() {
    let backend = Arc::new(FakeDockerBackend::default());
    let adapter = BrowserRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let error = adapter
        .execute(&operation(RuntimeOperation::LaunchBrowser(
            BrowserRuntimeLaunchRequest {
                session_id: Uuid::now_v7(),
                browser_context_id: None,
                features: BrowserRuntimeFeatures {
                    extension_version_ids: vec![Uuid::now_v7()],
                    ..Default::default()
                },
            },
        )))
        .await
        .unwrap_err();

    assert_eq!(error.code, ExecutionErrorCode::AdapterFailed);
    assert!(backend.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn lifecycle_derives_owned_target_and_reports_exists_absent_and_completed() {
    let backend = Arc::new(FakeDockerBackend::existing());
    let adapter = BrowserRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let session_id = Uuid::now_v7();

    assert_eq!(
        adapter
            .execute(&lifecycle(session_id, ContainerLifecycleAction::Inspect))
            .await
            .unwrap(),
        RuntimeOperationResult::Exists
    );
    assert_eq!(
        adapter
            .execute(&lifecycle(session_id, ContainerLifecycleAction::Stop))
            .await
            .unwrap(),
        RuntimeOperationResult::Completed {
            exit_code: None,
            omitted_output_bytes: 0,
        }
    );
    assert_eq!(
        adapter
            .execute(&lifecycle(session_id, ContainerLifecycleAction::Inspect))
            .await
            .unwrap(),
        RuntimeOperationResult::Absent
    );
    let expected = format!("bpane-runtime-{}", session_id.simple());
    let calls = backend.calls.lock().unwrap();
    assert!(matches!(&calls[0], BackendCall::Inspect(name) if name == &expected));
    assert!(matches!(&calls[1], BackendCall::Stop(name) if name == &expected));
    assert!(matches!(&calls[2], BackendCall::Inspect(name) if name == &expected));
}

#[tokio::test]
async fn launch_failure_is_sanitized_and_removes_partial_container() {
    let backend = Arc::new(FakeDockerBackend::default());
    backend.fail_next("start", DockerBackendError::Failed);
    let adapter = BrowserRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let session_id = Uuid::now_v7();
    let error = adapter
        .execute(&operation(RuntimeOperation::LaunchBrowser(
            BrowserRuntimeLaunchRequest {
                session_id,
                browser_context_id: None,
                features: Default::default(),
            },
        )))
        .await
        .unwrap_err();

    assert_eq!(error.code, ExecutionErrorCode::AdapterFailed);
    let calls = backend.calls.lock().unwrap();
    assert_eq!(calls.len(), 4);
    let expected = format!("bpane-runtime-{}", session_id.simple());
    assert!(matches!(&calls[3], BackendCall::Remove(name) if name == &expected));
    assert!(!error.to_string().contains("Docker"));
}

#[tokio::test]
async fn bollard_backend_uses_only_owned_container_lifecycle_routes() {
    type Calls = Arc<Mutex<Vec<(Method, String, Vec<u8>)>>>;

    async fn docker_api(
        State(calls): State<Calls>,
        method: Method,
        uri: Uri,
        body: Bytes,
    ) -> Response {
        calls
            .lock()
            .unwrap()
            .push((method.clone(), uri.to_string(), body.to_vec()));
        if method == Method::DELETE {
            return (
                StatusCode::NOT_FOUND,
                axum::Json(serde_json::json!({ "message": "not found" })),
            )
                .into_response();
        }
        if method == Method::POST && uri.path().ends_with("/containers/create") {
            return (
                StatusCode::CREATED,
                axum::Json(serde_json::json!({ "Id": "container-id", "Warnings": [] })),
            )
                .into_response();
        }
        if method == Method::POST && uri.path().ends_with("/start") {
            return StatusCode::NO_CONTENT.into_response();
        }
        StatusCode::NOT_FOUND.into_response()
    }

    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new().fallback(docker_api).with_state(calls.clone());
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let adapter = BrowserRuntimeDockerAdapter::connect(
        config(),
        &format!("http://{address}"),
        Duration::from_secs(2),
    )
    .unwrap();
    let session_id = Uuid::now_v7();

    assert_eq!(
        adapter
            .execute(&operation(RuntimeOperation::LaunchBrowser(
                BrowserRuntimeLaunchRequest {
                    session_id,
                    browser_context_id: None,
                    features: Default::default(),
                },
            )))
            .await
            .unwrap(),
        RuntimeOperationResult::Accepted
    );

    let calls = calls.lock().unwrap();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[0].0, Method::DELETE);
    assert!(calls[0].1.contains(&format!(
        "/containers/bpane-runtime-{}?",
        session_id.simple()
    )));
    assert_eq!(calls[1].0, Method::POST);
    assert!(calls[1].1.contains("/containers/create?"));
    assert!(calls[1]
        .1
        .contains(&format!("name=bpane-runtime-{}", session_id.simple())));
    let create: serde_json::Value = serde_json::from_slice(&calls[1].2).unwrap();
    assert_eq!(create["HostConfig"]["Privileged"], false);
    assert!(create["HostConfig"].get("Binds").is_none());
    assert_eq!(create["HostConfig"]["Mounts"][0]["Type"], "volume");
    assert_eq!(calls[2].0, Method::POST);
    assert!(calls[2].1.ends_with(&format!(
        "/containers/bpane-runtime-{}/start",
        session_id.simple()
    )));
}

#[tokio::test]
async fn unsupported_families_remain_fail_closed() {
    let adapter =
        BrowserRuntimeDockerAdapter::with_backend(config(), Arc::new(FakeDockerBackend::default()))
            .unwrap();
    let error = adapter
        .execute(&operation(RuntimeOperation::VolumeLifecycle(
            VolumeLifecycleRequest {
                operation_kind: RuntimeOperationKind::BrowserRuntime,
                resource_id: Uuid::now_v7(),
                action: VolumeLifecycleAction::Inspect,
            },
        )))
        .await
        .unwrap_err();
    assert_eq!(error.code, ExecutionErrorCode::AdapterUnavailable);
}

#[test]
fn rejects_mutable_images_escaping_socket_paths_and_unsafe_endpoints() {
    let backend: Arc<dyn DockerContainerApi> = Arc::new(FakeDockerBackend::default());
    let mut mutable_image = config();
    mutable_image.image = "browserpane/host:latest".to_string();
    assert_eq!(
        BrowserRuntimeDockerAdapter::with_backend(mutable_image, Arc::clone(&backend))
            .unwrap_err()
            .code,
        ExecutionErrorCode::AdapterFailed
    );

    let mut escaping_socket = config();
    escaping_socket.socket_path_root = "/run/bpane/../tmp/sockets".to_string();
    assert_eq!(
        BrowserRuntimeDockerAdapter::with_backend(escaping_socket, Arc::clone(&backend))
            .unwrap_err()
            .code,
        ExecutionErrorCode::AdapterFailed
    );

    let mut ambiguous_prefixes = config();
    ambiguous_prefixes.browser_context_volume_prefix =
        ambiguous_prefixes.session_data_volume_prefix.clone();
    assert_eq!(
        BrowserRuntimeDockerAdapter::with_backend(ambiguous_prefixes, Arc::clone(&backend))
            .unwrap_err()
            .code,
        ExecutionErrorCode::AdapterFailed
    );

    let extension_id = Uuid::now_v7();
    let mut unsafe_extension = config();
    unsafe_extension.extensions.insert(
        extension_id,
        BrowserRuntimeExtensionConfig {
            extension_version_id: extension_id,
            install_path: "/home/bpane/extensions/one,/tmp/unapproved".to_string(),
        },
    );
    assert_eq!(
        BrowserRuntimeDockerAdapter::with_backend(unsafe_extension, Arc::clone(&backend))
            .unwrap_err()
            .code,
        ExecutionErrorCode::AdapterFailed
    );

    assert_eq!(
        BrowserRuntimeDockerAdapter::connect(
            config(),
            "http://user:secret@docker-proxy:2375",
            Duration::from_secs(1),
        )
        .unwrap_err()
        .code,
        ExecutionErrorCode::AdapterFailed
    );
}
