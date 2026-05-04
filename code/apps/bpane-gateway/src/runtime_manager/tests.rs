use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use serde_json::{json, Value};
use uuid::Uuid;

use super::*;
use crate::session_control::SessionOwner;
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
        idle_generation: 0,
    };

    let args = manager.docker_run_args(&lease, &[]).unwrap();

    assert!(args.contains(&"deploy_agent-socket:/run/bpane/sessions".to_string()));
    assert!(args.contains(
        &"deploy_bpane-session-data-019db438c74a7ef2810c792e298faf11:/run/bpane/session"
            .to_string()
    ));
    assert!(!args.contains(&"deploy_agent-socket:/run/bpane".to_string()));
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
