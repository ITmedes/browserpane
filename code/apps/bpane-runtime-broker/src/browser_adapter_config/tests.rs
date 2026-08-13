use super::*;

fn settings() -> BrowserAdapterSettings {
    BrowserAdapterSettings {
        mode: RuntimeExecutorMode::DockerBrowser,
        docker_api_url: Some("http://docker-proxy:2375".to_string()),
        image: Some(format!(
            "registry.example/browserpane/host@sha256:{}",
            "a".repeat(64)
        )),
        network: Some("bpane-internal".to_string()),
        socket_volume: Some("agent-socket".to_string()),
        session_data_volume_prefix: "bpane-session-data".to_string(),
        browser_context_volume_prefix: "bpane-browser-context".to_string(),
        container_name_prefix: "bpane-runtime".to_string(),
        socket_mount_root: "/run/bpane".to_string(),
        socket_path_root: "/run/bpane/sessions".to_string(),
        session_data_root: "/run/bpane/session".to_string(),
        extension_registry_file: None,
        docker_timeout_secs: 30,
    }
}

#[test]
fn rejecting_mode_denies_ignored_adapter_configuration() {
    let mut candidate = settings();
    candidate.mode = RuntimeExecutorMode::Rejecting;
    assert!(candidate.build_executor().is_err());

    let rejecting = BrowserAdapterSettings {
        mode: RuntimeExecutorMode::Rejecting,
        docker_api_url: None,
        image: None,
        network: None,
        socket_volume: None,
        extension_registry_file: None,
        ..settings()
    };
    rejecting.build_executor().unwrap();
}

#[test]
fn docker_mode_requires_complete_bounded_configuration() {
    settings().build_executor().unwrap();

    let mut missing_image = settings();
    missing_image.image = None;
    assert!(missing_image.build_executor().is_err());

    let mut excessive_timeout = settings();
    excessive_timeout.docker_timeout_secs = 301;
    assert!(excessive_timeout.build_executor().is_err());
}

#[test]
fn extension_registry_is_bounded_unique_and_startup_scoped() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("extensions.json");
    let extension_id = Uuid::now_v7();
    std::fs::write(
        &path,
        serde_json::json!({
            "version": 1,
            "extensions": [{
                "extension_version_id": extension_id,
                "install_path": "/home/bpane/extensions/approved"
            }]
        })
        .to_string(),
    )
    .unwrap();
    let loaded = load_extension_registry(Some(&path)).unwrap();
    assert_eq!(
        loaded[&extension_id].install_path,
        "/home/bpane/extensions/approved"
    );

    std::fs::write(&path, r#"{"version":1,"extensions":[]}"#).unwrap();
    assert_eq!(
        loaded.len(),
        1,
        "loaded registry must remain an immutable snapshot"
    );
}

#[test]
fn extension_registry_rejects_duplicates_unknown_fields_and_unsafe_paths() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("extensions.json");
    let extension_id = Uuid::now_v7();
    std::fs::write(
        &path,
        serde_json::json!({
            "version": 1,
            "extensions": [
                {"extension_version_id": extension_id, "install_path": "/one"},
                {"extension_version_id": extension_id, "install_path": "/two"}
            ]
        })
        .to_string(),
    )
    .unwrap();
    assert!(load_extension_registry(Some(&path)).is_err());

    std::fs::write(&path, r#"{"version":1,"extensions":[],"extra":true}"#).unwrap();
    assert!(load_extension_registry(Some(&path)).is_err());

    std::fs::write(
        &path,
        serde_json::json!({
            "version": 1,
            "extensions": [{
                "extension_version_id": Uuid::now_v7(),
                "install_path": "relative/path"
            }]
        })
        .to_string(),
    )
    .unwrap();
    let mut candidate = settings();
    candidate.extension_registry_file = Some(path);
    assert!(candidate.build_executor().is_err());
}
