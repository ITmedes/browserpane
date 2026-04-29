use std::sync::Arc;
use std::time::Duration;

use uuid::Uuid;

use super::*;

fn docker_config() -> DockerRuntimeConfig {
    DockerRuntimeConfig {
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
