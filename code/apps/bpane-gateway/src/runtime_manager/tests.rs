use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use super::docker::DockerLeaseState;
use super::*;
use crate::auth::AuthenticatedPrincipal;
use crate::credentials::provider::{CredentialProviderBackend, ResolvedCredentialSecret};
use crate::credentials::{
    CredentialBindingProvider, CredentialInjectionMode, CredentialProvider,
    CredentialProviderError, PersistCredentialBindingRequest, StoreCredentialSecretRequest,
    StoredCredentialSecret,
};
use crate::session_control::{
    BrowserContextPersistenceMode, CreateSessionRequest, EgressCustomCaConfig, EgressProfileState,
    EgressProxyConfig, EgressTrafficObservationConfig, EgressTrafficObservationMode,
    PersistBrowserContextRequest, PersistEgressProfileRequest, SessionBrowserContextMode,
    SessionBrowserContextRequest, SessionBrowserContextResource, SessionGeolocation,
    SessionLifecycleState, SessionNetworkIdentity, SessionOwner, SessionOwnerMode,
    SessionRecordingPolicy, SessionStore, SessionViewport, StoredEgressProfile, StoredSession,
};
use crate::session_files::{
    SessionFileBindingMode, SessionFileBindingState, StoredSessionFileBinding,
};

fn docker_config() -> DockerRuntimeConfig {
    DockerRuntimeConfig {
        docker_bin: "docker".to_string(),
        image: "deploy-host".to_string(),
        network: "deploy_bpane-internal".to_string(),
        socket_volume: "deploy_agent-socket".to_string(),
        session_data_volume_prefix: "deploy_bpane-session-data".to_string(),
        container_name_prefix: "bpane-runtime".to_string(),
        socket_root: "/run/bpane/sessions".to_string(),
        session_data_root: "/run/bpane/session".to_string(),
        cdp_proxy_port: 9223,
        shm_size: "128m".to_string(),
        start_timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(300),
        max_active_runtimes: 2,
        max_starting_runtimes: 1,
        seccomp_unconfined: true,
        env_file: None,
    }
}

fn docker_profile(max_runtime_sessions: usize) -> RuntimeProfile {
    RuntimeProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    }
}

fn test_principal(subject: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        subject: subject.to_string(),
        issuer: "https://issuer.example".to_string(),
        display_name: Some(subject.to_string()),
        client_id: None,
        safe_claims: Default::default(),
    }
}

#[derive(Debug)]
struct FixedCredentialProviderBackend {
    payload: Value,
}

#[async_trait::async_trait]
impl CredentialProviderBackend for FixedCredentialProviderBackend {
    async fn store_secret(
        &self,
        request: StoreCredentialSecretRequest,
    ) -> Result<StoredCredentialSecret, CredentialProviderError> {
        Ok(StoredCredentialSecret {
            external_ref: request
                .external_ref
                .unwrap_or_else(|| format!("test/{}", request.binding_id)),
        })
    }

    async fn resolve_secret(
        &self,
        _external_ref: &str,
    ) -> Result<ResolvedCredentialSecret, CredentialProviderError> {
        Ok(ResolvedCredentialSecret {
            payload: self.payload.clone(),
        })
    }
}

#[tokio::test]
async fn static_single_runtime_reuses_same_session_assignment() {
    let manager = SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
        agent_socket_path: "/tmp/bpane.sock".to_string(),
        cdp_endpoint: Some("http://host:9223".to_string()),
        idle_timeout: Duration::from_secs(300),
    })
    .unwrap();
    let session_id = Uuid::now_v7();

    let first = manager.resolve(session_id).await.unwrap();
    let second = manager.resolve(session_id).await.unwrap();

    assert_eq!(first, second);
    assert_eq!(first.agent_socket_path, "/tmp/bpane.sock");
    assert_eq!(
        manager.profile().compatibility_mode,
        "legacy_single_runtime"
    );
    assert_eq!(
        manager
            .describe_session_runtime(session_id)
            .cdp_endpoint
            .as_deref(),
        Some("http://host:9223")
    );
}

#[tokio::test]
async fn static_single_runtime_blocks_parallel_session_assignment() {
    let manager = SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
        agent_socket_path: "/tmp/bpane.sock".to_string(),
        cdp_endpoint: None,
        idle_timeout: Duration::from_secs(300),
    })
    .unwrap();
    let session_a = Uuid::now_v7();
    let session_b = Uuid::now_v7();

    manager.resolve(session_a).await.unwrap();
    let error = manager.resolve(session_b).await.unwrap_err();

    assert_eq!(
        error,
        RuntimeManagerError::RuntimeBusy {
            active_session_id: session_a,
        }
    );
}

#[tokio::test]
async fn static_single_runtime_release_allows_next_session() {
    let manager = SessionRuntimeManager::new(RuntimeManagerConfig::StaticSingle {
        agent_socket_path: "/tmp/bpane.sock".to_string(),
        cdp_endpoint: None,
        idle_timeout: Duration::from_secs(300),
    })
    .unwrap();
    let session_a = Uuid::now_v7();
    let session_b = Uuid::now_v7();

    manager.resolve(session_a).await.unwrap();
    manager.release(session_a).await;
    let resolved = manager.resolve(session_b).await.unwrap();

    assert_eq!(resolved.session_id, session_b);
}

#[test]
fn docker_runtime_requires_core_configuration() {
    let error = SessionRuntimeManager::new(RuntimeManagerConfig::DockerPool(DockerRuntimeConfig {
        image: String::new(),
        ..docker_config()
    }))
    .err()
    .unwrap();

    assert!(matches!(
        error,
        RuntimeManagerError::InvalidConfiguration(_)
    ));
}

#[test]
fn docker_runtime_rejects_root_runtime_mounts() {
    let socket_error =
        SessionRuntimeManager::new(RuntimeManagerConfig::DockerPool(DockerRuntimeConfig {
            socket_root: "/".to_string(),
            ..docker_config()
        }))
        .err()
        .unwrap();
    assert_eq!(
        socket_error,
        RuntimeManagerError::InvalidConfiguration(
            "docker runtime backend requires socket_root below /".to_string(),
        )
    );

    let data_error =
        SessionRuntimeManager::new(RuntimeManagerConfig::DockerPool(DockerRuntimeConfig {
            session_data_root: "/".to_string(),
            ..docker_config()
        }))
        .err()
        .unwrap();
    assert_eq!(
        data_error,
        RuntimeManagerError::InvalidConfiguration(
            "docker runtime backend requires session_data_root below /".to_string(),
        )
    );
}

#[test]
fn docker_runtime_validates_starting_capacity_limit() {
    let error = SessionRuntimeManager::new(RuntimeManagerConfig::DockerPool(DockerRuntimeConfig {
        max_starting_runtimes: 3,
        max_active_runtimes: 2,
        ..docker_config()
    }))
    .err()
    .unwrap();

    assert!(matches!(
        error,
        RuntimeManagerError::InvalidConfiguration(_)
    ));
}

#[test]
fn docker_pool_profile_exposes_runtime_capacity() {
    let manager =
        SessionRuntimeManager::new(RuntimeManagerConfig::DockerPool(docker_config())).unwrap();

    assert_eq!(manager.profile().compatibility_mode, "session_runtime_pool");
    assert_eq!(manager.profile().max_runtime_sessions, 2);
    assert!(!manager.profile().supports_legacy_global_routes);
    assert_eq!(
        manager
            .describe_session_runtime(Uuid::nil())
            .cdp_endpoint
            .as_deref(),
        Some("http://bpane-runtime-00000000000000000000000000000000:9223")
    );
}

#[test]
fn docker_runtime_names_and_sockets_are_session_scoped() {
    let manager = Arc::new(
        DockerRuntimeManager::new(
            docker_config(),
            RuntimeProfile {
                runtime_binding: "docker_runtime_pool".to_string(),
                compatibility_mode: "session_runtime_pool".to_string(),
                max_runtime_sessions: 2,
                supports_legacy_global_routes: false,
                supports_session_extensions: true,
            },
        )
        .unwrap(),
    );
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();

    assert_eq!(
        manager.socket_path_for_session(session_id),
        "/run/bpane/sessions/019db438-c74a-7ef2-810c-792e298faf11.sock"
    );
    assert_eq!(
        manager.container_name_for_session(session_id),
        format!("bpane-runtime-{}", session_id.as_simple())
    );
    assert_eq!(
        manager.cdp_endpoint_for_session(session_id),
        format!("http://bpane-runtime-{}:9223", session_id.as_simple())
    );
}

#[test]
fn docker_runtime_parses_volume_size_units() {
    assert_eq!(docker::parse_docker_size_bytes("176B"), Some(176));
    assert_eq!(docker::parse_docker_size_bytes("4.62MB"), Some(4_620_000));
    assert_eq!(
        docker::parse_docker_size_bytes("1.5GiB"),
        Some(1_610_612_736)
    );
    assert_eq!(docker::parse_docker_size_bytes("N/A"), None);
    assert_eq!(docker::parse_docker_size_bytes("unknown"), None);
}

#[test]
fn docker_runtime_launch_separates_socket_and_session_data_mounts() {
    let manager = DockerRuntimeManager::new(
        docker_config(),
        RuntimeProfile {
            runtime_binding: "docker_runtime_pool".to_string(),
            compatibility_mode: "session_runtime_pool".to_string(),
            max_runtime_sessions: 2,
            supports_legacy_global_routes: false,
            supports_session_extensions: true,
        },
    )
    .unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let lease = RuntimeLease {
        session_id,
        agent_socket_path: manager.socket_path_for_session(session_id),
        container_name: Some(manager.container_name_for_session(session_id)),
        browser_context_id: None,
        discard_session_data_on_release: false,
        idle_generation: 0,
    };

    let args = manager.docker_run_args(&lease, &[]).unwrap();

    assert!(args.contains(&"deploy_agent-socket:/run/bpane".to_string()));
    assert!(args.contains(
        &"deploy_bpane-session-data-019db438c74a7ef2810c792e298faf11:/run/bpane/session"
            .to_string()
    ));
    assert!(!args.contains(&"deploy_agent-socket:/run/bpane/sessions".to_string()));
    assert!(args.contains(
        &"BPANE_SOCKET_PATH=/run/bpane/sessions/019db438-c74a-7ef2-810c-792e298faf11.sock"
            .to_string()
    ));
    assert!(args.contains(&"BPANE_SESSION_DATA_DIR=/run/bpane/session".to_string()));
    assert!(args.contains(&"BPANE_PROFILE_DIR=/run/bpane/session/chromium".to_string()));
    assert!(args.contains(&"BPANE_UPLOAD_DIR=/run/bpane/session/uploads".to_string()));
    assert!(args.contains(&"BPANE_DOWNLOAD_DIR=/run/bpane/session/downloads".to_string()));
    assert!(args.contains(&"BPANE_SESSION_FILE_MOUNTS_DIR=/run/bpane/session/mounts".to_string()));
    assert!(args.contains(
        &"BPANE_SESSION_FILE_BINDINGS_MANIFEST=/run/bpane/session/session-file-bindings.json"
            .to_string()
    ));
}

#[test]
fn docker_runtime_maps_network_identity_to_launch_env() {
    let manager = DockerRuntimeManager::new(docker_config(), docker_profile(2)).unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let profile_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf12").unwrap();
    let proxy_auth_binding_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf13").unwrap();
    let now = Utc::now();
    let session = StoredSession {
        id: session_id,
        state: SessionLifecycleState::Ready,
        project_id: None,
        admission: crate::session_control::ProjectAdmissionDecision::owner_scope_unbounded(now),
        template_id: None,
        browser_context: SessionBrowserContextResource {
            mode: SessionBrowserContextMode::Fresh,
            context_id: None,
        },
        network_identity: SessionNetworkIdentity {
            locale: Some("de-DE".to_string()),
            languages: vec!["de-DE".to_string(), "en-US".to_string()],
            timezone: Some("Europe/Berlin".to_string()),
            geolocation: Some(SessionGeolocation {
                latitude: 52.52,
                longitude: 13.405,
                accuracy_meters: Some(100.0),
            }),
            user_agent: Some("BrowserPane Test/1.0".to_string()),
            browser_identity: Some("desktop-chromium-stable".to_string()),
            egress_profile_id: Some(profile_id),
        },
        owner_mode: SessionOwnerMode::Collaborative,
        viewport: SessionViewport::default(),
        owner: SessionOwner {
            subject: "owner".to_string(),
            issuer: "https://issuer.example".to_string(),
            display_name: None,
        },
        automation_delegate: None,
        idle_timeout_sec: None,
        labels: HashMap::new(),
        integration_context: None,
        extensions: Vec::new(),
        recording: SessionRecordingPolicy::default(),
        created_at: now,
        updated_at: now,
        queued_at: None,
        runtime_started_at: Some(now),
        runtime_usage_ms: 0,
        egress_rx_bytes: 0,
        egress_tx_bytes: 0,
        runtime_released_at: None,
        stopped_at: None,
    };
    let profile = StoredEgressProfile {
        id: profile_id,
        owner_subject: "owner".to_string(),
        owner_issuer: "https://issuer.example".to_string(),
        name: "eu-support-egress".to_string(),
        description: None,
        labels: HashMap::new(),
        proxy: Some(EgressProxyConfig {
            url: "https://proxy.example:8443".to_string(),
            credential_binding_id: Some(proxy_auth_binding_id),
        }),
        bypass_rules: vec!["localhost".to_string(), "*.internal.example".to_string()],
        custom_ca: Some(EgressCustomCaConfig {
            certificate_ref: "file:///workspace/dev/egress-ca.pem".to_string(),
            display_name: Some("EU support CA".to_string()),
        }),
        traffic_observation: EgressTrafficObservationConfig {
            mode: EgressTrafficObservationMode::TlsIntercept,
            sensitive_log_sink_ref: Some("siem://browserpane/eu-support".to_string()),
            sensitive_log_sink_display_name: Some("EU support SIEM".to_string()),
        },
        state: EgressProfileState::Ready,
        created_at: now,
        updated_at: now,
    };
    let lease = RuntimeLease {
        session_id,
        agent_socket_path: manager.socket_path_for_session(session_id),
        container_name: Some(manager.container_name_for_session(session_id)),
        browser_context_id: None,
        discard_session_data_on_release: false,
        idle_generation: 0,
    };

    let launch_options = DockerRuntimeManager::network_identity_launch_options(
        &session,
        Some(&profile),
        &manager.egress_custom_ca_bundle_path_for_session(),
        &manager.egress_proxy_auth_config_path_for_session(),
    );
    let env = launch_options
        .env
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>();

    assert_eq!(env.get("LANG").map(String::as_str), Some("de_DE.UTF-8"));
    assert_eq!(env.get("LC_ALL").map(String::as_str), Some("de_DE.UTF-8"));
    assert_eq!(env.get("LANGUAGE").map(String::as_str), Some("de-DE:en-US"));
    assert_eq!(env.get("TZ").map(String::as_str), Some("Europe/Berlin"));
    assert_eq!(
        env.get("BPANE_CHROMIUM_ACCEPT_LANG").map(String::as_str),
        Some("de-DE,en-US")
    );
    assert_eq!(
        env.get("BPANE_CHROMIUM_USER_AGENT").map(String::as_str),
        Some("BrowserPane Test/1.0")
    );
    assert_eq!(
        env.get("BPANE_CHROMIUM_PROXY_SERVER").map(String::as_str),
        Some("https://proxy.example:8443")
    );
    assert_eq!(
        env.get("BPANE_CHROMIUM_PROXY_AUTH_FILE")
            .map(String::as_str),
        Some("/run/bpane/session/egress/proxy-auth.json")
    );
    assert_eq!(
        env.get("BPANE_URL").map(String::as_str),
        Some("about:blank")
    );
    assert_eq!(
        env.get("BPANE_CHROMIUM_PROXY_BYPASS_LIST")
            .map(String::as_str),
        Some("localhost;*.internal.example")
    );
    assert_eq!(
        env.get("BPANE_EGRESS_OBSERVATION_MODE").map(String::as_str),
        Some("tls_intercept")
    );
    assert_eq!(
        env.get("BPANE_CHROMIUM_TRUSTED_CA_BUNDLE")
            .map(String::as_str),
        Some("/run/bpane/session/egress/custom-ca.pem")
    );
    assert_eq!(
        env.get("BPANE_CHROMIUM_TRUSTED_CA_NAME")
            .map(String::as_str),
        Some("EU support CA")
    );
    assert_eq!(
        launch_options.egress_custom_ca_ref.as_deref(),
        Some("file:///workspace/dev/egress-ca.pem")
    );
    let labels = launch_options
        .labels
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>();
    assert_eq!(
        labels
            .get("browserpane.egress_profile_id")
            .map(String::as_str),
        Some("019db438-c74a-7ef2-810c-792e298faf12")
    );
    assert_eq!(
        labels
            .get("browserpane.egress_proxy_configured")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        labels
            .get("browserpane.egress_proxy_auth_configured")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        labels
            .get("browserpane.egress_bypass_rule_count")
            .map(String::as_str),
        Some("2")
    );
    assert_eq!(
        labels
            .get("browserpane.egress_custom_ca_configured")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        labels
            .get("browserpane.egress_observation_mode")
            .map(String::as_str),
        Some("tls_intercept")
    );
    assert_eq!(
        labels
            .get("browserpane.egress_tls_interception_enabled")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        labels
            .get("browserpane.egress_sensitive_log_sink_configured")
            .map(String::as_str),
        Some("true")
    );
    assert_eq!(
        launch_options.egress_observer.as_ref().map(|summary| (
            &summary.profile_id,
            summary.observation_mode,
            summary.proxy_configured,
            summary.tls_interception_enabled,
        )),
        Some((
            &profile_id,
            EgressTrafficObservationMode::TlsIntercept,
            true,
            true,
        ))
    );
    let geolocation: Value =
        serde_json::from_str(env.get("BPANE_SESSION_GEOLOCATION").unwrap()).unwrap();
    assert_eq!(geolocation["latitude"], json!(52.52));
    assert_eq!(geolocation["longitude"], json!(13.405));

    let args = manager
        .docker_run_args_with_launch_options(&lease, &[], &launch_options)
        .unwrap();
    assert!(args.contains(&"BPANE_CHROMIUM_USER_AGENT=BrowserPane Test/1.0".to_string()));
    assert!(args.contains(&"BPANE_CHROMIUM_PROXY_SERVER=https://proxy.example:8443".to_string()));
    assert!(args.contains(
        &"BPANE_CHROMIUM_PROXY_AUTH_FILE=/run/bpane/session/egress/proxy-auth.json".to_string()
    ));
    assert!(args.contains(&"BPANE_URL=about:blank".to_string()));
    assert!(args.contains(
        &"browserpane.egress_profile_id=019db438-c74a-7ef2-810c-792e298faf12".to_string()
    ));
    assert!(!args.iter().any(|arg| arg.contains("secret-password")));
}

#[tokio::test]
async fn docker_runtime_resolves_secret_backed_proxy_auth_for_launch() {
    let manager = Arc::new(DockerRuntimeManager::new(docker_config(), docker_profile(2)).unwrap());
    let store = SessionStore::in_memory_with_config(docker_profile(2));
    let principal = test_principal("owner");
    let binding_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf20").unwrap();
    manager.attach_session_store(store.clone()).await;
    manager
        .attach_credential_provider(Some(Arc::new(CredentialProvider::new(Arc::new(
            FixedCredentialProviderBackend {
                payload: json!({
                    "username": "proxy-user",
                    "password": "secret-password"
                }),
            },
        )))))
        .await;
    store
        .create_credential_binding(
            &principal,
            PersistCredentialBindingRequest {
                id: binding_id,
                name: "support-proxy-auth".to_string(),
                provider: CredentialBindingProvider::VaultKvV2,
                external_ref: "test/proxy-auth".to_string(),
                namespace: None,
                allowed_origins: Vec::new(),
                injection_mode: CredentialInjectionMode::FormFill,
                totp: None,
                labels: HashMap::from([("purpose".to_string(), "egress_proxy_auth".to_string())]),
            },
        )
        .await
        .unwrap();
    let profile = store
        .create_egress_profile(
            &principal,
            PersistEgressProfileRequest {
                name: "authenticated-proxy".to_string(),
                description: None,
                labels: HashMap::new(),
                proxy: Some(EgressProxyConfig {
                    url: "https://proxy.example:8443".to_string(),
                    credential_binding_id: Some(binding_id),
                }),
                bypass_rules: Vec::new(),
                custom_ca: None,
                traffic_observation: EgressTrafficObservationConfig::default(),
                state: EgressProfileState::Ready,
            },
        )
        .await
        .unwrap();
    let session = store
        .create_session(
            &principal,
            CreateSessionRequest {
                project_id: None,
                network_identity: Some(SessionNetworkIdentity {
                    egress_profile_id: Some(profile.id),
                    ..Default::default()
                }),
                ..Default::default()
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let launch_options = manager
        .session_network_identity_launch_options(session.id)
        .await
        .unwrap();
    let env = launch_options
        .env
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>();
    assert_eq!(
        env.get("BPANE_CHROMIUM_PROXY_AUTH_FILE")
            .map(String::as_str),
        Some("/run/bpane/session/egress/proxy-auth.json")
    );
    let material = launch_options.egress_proxy_auth.as_ref().unwrap();
    assert_eq!(material.binding_id, binding_id);
    assert_eq!(
        material.target_path,
        "/run/bpane/session/egress/proxy-auth.json"
    );
    let payload: Value = serde_json::from_slice(&material.payload).unwrap();
    assert_eq!(payload["username"], "proxy-user");
    assert_eq!(payload["password"], "secret-password");
    let labels = launch_options
        .labels
        .iter()
        .cloned()
        .collect::<HashMap<_, _>>();
    assert_eq!(
        labels
            .get("browserpane.egress_proxy_auth_configured")
            .map(String::as_str),
        Some("true")
    );
    assert!(!launch_options
        .env
        .iter()
        .any(|(_, value)| value.contains("secret-password")));
    assert!(!launch_options
        .labels
        .iter()
        .any(|(_, value)| value.contains("secret-password")));
}

#[test]
fn docker_runtime_mounts_reusable_browser_context_profile_volume() {
    let manager = DockerRuntimeManager::new(docker_config(), docker_profile(2)).unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let context_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf22").unwrap();
    let lease = RuntimeLease {
        session_id,
        agent_socket_path: manager.socket_path_for_session(session_id),
        container_name: Some(manager.container_name_for_session(session_id)),
        browser_context_id: Some(context_id),
        discard_session_data_on_release: false,
        idle_generation: 0,
    };

    let run_args = manager.docker_run_args(&lease, &[]).unwrap();
    let init_args = manager.docker_initialize_session_data_args(&lease);
    let context_profile_mount =
        "deploy_bpane-session-data-browser-context-019db438c74a7ef2810c792e298faf22:/run/bpane/session/chromium"
            .to_string();

    assert_eq!(
        manager.browser_context_profile_volume_for_context(context_id),
        "deploy_bpane-session-data-browser-context-019db438c74a7ef2810c792e298faf22"
    );
    assert!(run_args.contains(
        &"deploy_bpane-session-data-019db438c74a7ef2810c792e298faf11:/run/bpane/session"
            .to_string()
    ));
    assert!(run_args.contains(&context_profile_mount));
    assert!(run_args.contains(&"BPANE_PROFILE_DIR=/run/bpane/session/chromium".to_string()));
    assert!(run_args.contains(&"BPANE_UPLOAD_DIR=/run/bpane/session/uploads".to_string()));
    assert!(run_args.contains(&"BPANE_DOWNLOAD_DIR=/run/bpane/session/downloads".to_string()));
    assert!(
        run_args.contains(&"BPANE_SESSION_FILE_MOUNTS_DIR=/run/bpane/session/mounts".to_string())
    );

    assert!(init_args.contains(&context_profile_mount));
    let materialize_args = manager.docker_materialize_file_args(
        session_id,
        "/run/bpane/session/mounts/inputs/input.csv",
        "0444",
    );
    assert!(!materialize_args.contains(&context_profile_mount));
    assert!(materialize_args.contains(
        &"deploy_bpane-session-data-019db438c74a7ef2810c792e298faf11:/run/bpane/session"
            .to_string()
    ));
}

#[test]
fn docker_runtime_materializes_session_file_bindings_inside_session_data_volume() {
    let manager = DockerRuntimeManager::new(
        docker_config(),
        RuntimeProfile {
            runtime_binding: "docker_runtime_pool".to_string(),
            compatibility_mode: "session_runtime_pool".to_string(),
            max_runtime_sessions: 2,
            supports_legacy_global_routes: false,
            supports_session_extensions: true,
        },
    )
    .unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let binding = StoredSessionFileBinding {
        id: Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf12").unwrap(),
        session_id,
        workspace_id: Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf13").unwrap(),
        file_id: Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf14").unwrap(),
        file_name: "input.csv".to_string(),
        media_type: Some("text/csv".to_string()),
        byte_count: 12,
        sha256_hex: "abc123".to_string(),
        provenance: Some(json!({ "source": "test" })),
        artifact_ref: "local_fs:workspace/input.csv".to_string(),
        mount_path: "inputs/input.csv".to_string(),
        mode: SessionFileBindingMode::ReadOnly,
        state: SessionFileBindingState::Pending,
        error: None,
        labels: HashMap::from([("suite".to_string(), "unit".to_string())]),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };

    assert_eq!(
        manager.materialized_path_for_binding(&binding),
        "/run/bpane/session/mounts/inputs/input.csv"
    );

    let args = manager.docker_materialize_file_args(
        session_id,
        "/run/bpane/session/mounts/inputs/input.csv",
        "0444",
    );
    assert!(args.contains(&"--network".to_string()));
    assert!(args.contains(&"none".to_string()));
    assert!(args.contains(
        &"deploy_bpane-session-data-019db438c74a7ef2810c792e298faf11:/run/bpane/session"
            .to_string()
    ));
    assert!(args.contains(&"BPANE_SESSION_DATA_DIR=/run/bpane/session".to_string()));
    assert!(args.contains(
        &"BPANE_MATERIALIZE_TARGET=/run/bpane/session/mounts/inputs/input.csv".to_string()
    ));
    assert!(args.contains(&"BPANE_MATERIALIZE_MODE=0444".to_string()));
    assert!(args.contains(&"--entrypoint".to_string()));
    assert!(args.contains(&"/bin/sh".to_string()));

    let manifest = manager
        .build_session_file_manifest(
            session_id,
            &SessionOwner {
                subject: "owner".to_string(),
                issuer: "https://issuer.example".to_string(),
                display_name: None,
            },
            &[binding],
        )
        .unwrap();
    let manifest: Value = serde_json::from_slice(&manifest).unwrap();
    assert_eq!(manifest["format_version"], 1);
    assert_eq!(manifest["owner"]["subject"], "owner");
    assert_eq!(manifest["mounts_root"], "/run/bpane/session/mounts");
    assert_eq!(manifest["bindings"][0]["source"], "workspace");
    assert_eq!(
        manifest["bindings"][0]["materialized_path"],
        "/run/bpane/session/mounts/inputs/input.csv"
    );
    assert_eq!(manifest["bindings"][0]["state"], "materialized");
}

#[test]
fn docker_runtime_initializes_session_data_volume_as_root() {
    let manager = DockerRuntimeManager::new(
        docker_config(),
        RuntimeProfile {
            runtime_binding: "docker_runtime_pool".to_string(),
            compatibility_mode: "session_runtime_pool".to_string(),
            max_runtime_sessions: 2,
            supports_legacy_global_routes: false,
            supports_session_extensions: true,
        },
    )
    .unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();

    let lease = RuntimeLease {
        session_id,
        agent_socket_path: manager.socket_path_for_session(session_id),
        container_name: Some(manager.container_name_for_session(session_id)),
        browser_context_id: None,
        discard_session_data_on_release: false,
        idle_generation: 0,
    };

    let args = manager.docker_initialize_session_data_args(&lease);

    assert!(args.contains(&"--network".to_string()));
    assert!(args.contains(&"none".to_string()));
    assert!(args.contains(
        &"deploy_bpane-session-data-019db438c74a7ef2810c792e298faf11:/run/bpane/session"
            .to_string()
    ));
    assert!(args.contains(&"BPANE_SESSION_DATA_DIR=/run/bpane/session".to_string()));
    assert!(args.contains(&"BPANE_PROFILE_DIR=/run/bpane/session/chromium".to_string()));
    assert!(args.contains(&"BPANE_UPLOAD_DIR=/run/bpane/session/uploads".to_string()));
    assert!(args.contains(&"BPANE_DOWNLOAD_DIR=/run/bpane/session/downloads".to_string()));
    assert!(args.contains(&"BPANE_SESSION_FILE_MOUNTS_DIR=/run/bpane/session/mounts".to_string()));
    assert!(args.contains(&"--user".to_string()));
    assert!(args.contains(&"0:0".to_string()));
    assert!(args.contains(&"--entrypoint".to_string()));
    assert!(args.contains(&"/bin/sh".to_string()));
}

#[tokio::test]
async fn docker_runtime_rejects_parallel_writer_for_reusable_browser_context() {
    let manager = Arc::new(DockerRuntimeManager::new(docker_config(), docker_profile(2)).unwrap());
    let store = SessionStore::in_memory_with_config(docker_profile(2));
    manager.attach_session_store(store.clone()).await;
    let principal = test_principal("owner");
    let context = store
        .create_browser_context(
            &principal,
            PersistBrowserContextRequest {
                id: None,
                name: "authenticated".to_string(),
                description: None,
                labels: HashMap::new(),
                persistence_mode: BrowserContextPersistenceMode::Reusable,
                retention_sec: None,
                max_profile_storage_bytes: None,
            },
        )
        .await
        .unwrap();
    let create_request = || CreateSessionRequest {
        project_id: None,
        browser_context: Some(SessionBrowserContextRequest {
            mode: SessionBrowserContextMode::Reusable,
            context_id: Some(context.id),
        }),
        ..Default::default()
    };
    let session_a = store
        .create_session(
            &principal,
            create_request(),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let session_b = store
        .create_session(
            &principal,
            create_request(),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    manager.leases.lock().await.insert(
        session_a.id,
        DockerLeaseState::Ready(RuntimeLease {
            session_id: session_a.id,
            agent_socket_path: manager.socket_path_for_session(session_a.id),
            container_name: Some(manager.container_name_for_session(session_a.id)),
            browser_context_id: Some(context.id),
            discard_session_data_on_release: false,
            idle_generation: 0,
        }),
    );

    let error = manager.resolve(session_b.id).await.unwrap_err();

    assert_eq!(
        error,
        RuntimeManagerError::BrowserContextInUse {
            browser_context_id: context.id,
            active_session_id: session_a.id,
        }
    );
}

#[tokio::test]
async fn docker_runtime_rejects_context_data_delete_while_context_is_active() {
    let manager = DockerRuntimeManager::new(docker_config(), docker_profile(2)).unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let context_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf22").unwrap();
    manager.leases.lock().await.insert(
        session_id,
        DockerLeaseState::Ready(RuntimeLease {
            session_id,
            agent_socket_path: manager.socket_path_for_session(session_id),
            container_name: Some(manager.container_name_for_session(session_id)),
            browser_context_id: Some(context_id),
            discard_session_data_on_release: false,
            idle_generation: 0,
        }),
    );

    let error = manager
        .delete_browser_context_data(context_id)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        RuntimeManagerError::BrowserContextInUse {
            browser_context_id: context_id,
            active_session_id: session_id,
        }
    );
}

#[tokio::test]
async fn docker_runtime_rejects_context_data_clone_while_source_is_active() {
    let manager = DockerRuntimeManager::new(docker_config(), docker_profile(2)).unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let source_context_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf22").unwrap();
    let target_context_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf33").unwrap();
    manager.leases.lock().await.insert(
        session_id,
        DockerLeaseState::Ready(RuntimeLease {
            session_id,
            agent_socket_path: manager.socket_path_for_session(session_id),
            container_name: Some(manager.container_name_for_session(session_id)),
            browser_context_id: Some(source_context_id),
            discard_session_data_on_release: false,
            idle_generation: 0,
        }),
    );

    let error = manager
        .clone_browser_context_data(source_context_id, target_context_id)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        RuntimeManagerError::BrowserContextInUse {
            browser_context_id: source_context_id,
            active_session_id: session_id,
        }
    );
}

#[tokio::test]
async fn docker_runtime_rejects_context_export_while_context_is_active() {
    let manager = DockerRuntimeManager::new(docker_config(), docker_profile(2)).unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let context_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf22").unwrap();
    manager.leases.lock().await.insert(
        session_id,
        DockerLeaseState::Ready(RuntimeLease {
            session_id,
            agent_socket_path: manager.socket_path_for_session(session_id),
            container_name: Some(manager.container_name_for_session(session_id)),
            browser_context_id: Some(context_id),
            discard_session_data_on_release: false,
            idle_generation: 0,
        }),
    );

    let error = manager
        .export_browser_context_profile_archive(context_id)
        .await
        .unwrap_err();

    assert_eq!(
        error,
        RuntimeManagerError::BrowserContextInUse {
            browser_context_id: context_id,
            active_session_id: session_id,
        }
    );
}

#[tokio::test]
async fn docker_runtime_rejects_context_import_while_context_is_active() {
    let manager = DockerRuntimeManager::new(docker_config(), docker_profile(2)).unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let context_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf22").unwrap();
    manager.leases.lock().await.insert(
        session_id,
        DockerLeaseState::Ready(RuntimeLease {
            session_id,
            agent_socket_path: manager.socket_path_for_session(session_id),
            container_name: Some(manager.container_name_for_session(session_id)),
            browser_context_id: Some(context_id),
            discard_session_data_on_release: false,
            idle_generation: 0,
        }),
    );

    let error = manager
        .import_browser_context_profile_archive(context_id, Some(b"profile"))
        .await
        .unwrap_err();

    assert_eq!(
        error,
        RuntimeManagerError::BrowserContextInUse {
            browser_context_id: context_id,
            active_session_id: session_id,
        }
    );
}

#[tokio::test]
async fn docker_runtime_reports_active_browser_context_writer() {
    let manager = DockerRuntimeManager::new(docker_config(), docker_profile(2)).unwrap();
    let session_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf11").unwrap();
    let context_id = Uuid::parse_str("019db438-c74a-7ef2-810c-792e298faf22").unwrap();
    manager.leases.lock().await.insert(
        session_id,
        DockerLeaseState::Starting {
            lease: RuntimeLease {
                session_id,
                agent_socket_path: manager.socket_path_for_session(session_id),
                container_name: Some(manager.container_name_for_session(session_id)),
                browser_context_id: Some(context_id),
                discard_session_data_on_release: false,
                idle_generation: 0,
            },
            notify: Arc::new(tokio::sync::Notify::new()),
        },
    );

    assert_eq!(
        manager.active_browser_context_session_id(context_id).await,
        Some(session_id)
    );
    assert_eq!(
        manager
            .active_browser_context_session_id(Uuid::now_v7())
            .await,
        None
    );
}
