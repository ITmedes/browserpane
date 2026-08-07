use clap::Parser;

use crate::config::Config;
use crate::session_control::SessionOwnerMode;
use crate::session_manager::SessionManagerConfig;

use super::builders::{
    build_recording_worker_config, build_session_manager_config, default_owner_mode,
    load_or_generate_shared_secret, session_file_retention_window, workflow_retention_window,
};

fn test_config() -> Config {
    Config::parse_from(["bpane-gateway"])
}

#[test]
fn shared_secret_rejects_too_short_hmac_secret() {
    let mut config = test_config();
    config.auth.hmac_secret = Some("00112233445566778899aabbccddee".to_string());

    let error = load_or_generate_shared_secret(&config).unwrap_err();

    assert!(error
        .to_string()
        .contains("HMAC secret must be at least 16 bytes"));
}

#[test]
fn docker_single_runtime_requires_image() {
    let mut config = test_config();
    config.runtime.backend = "docker_single".to_string();
    config.runtime.docker_network = Some("network".to_string());
    config.runtime.docker_socket_volume = Some("volume".to_string());

    let error = build_session_manager_config(&config).unwrap_err();

    assert!(error
        .to_string()
        .contains("--docker-runtime-image is required"));
}

#[test]
fn docker_pool_runtime_uses_configured_capacities() {
    let mut config = test_config();
    config.runtime.backend = "docker_pool".to_string();
    config.runtime.docker_image = Some("image".to_string());
    config.runtime.docker_network = Some("network".to_string());
    config.runtime.docker_socket_volume = Some("volume".to_string());
    config.runtime.max_active_runtimes = 4;
    config.runtime.max_starting_runtimes = 2;

    let manager_config = build_session_manager_config(&config).unwrap();

    let SessionManagerConfig::DockerPool(docker_config) = manager_config else {
        panic!("expected docker_pool session manager config");
    };
    assert_eq!(docker_config.max_active_runtimes, 4);
    assert_eq!(docker_config.max_starting_runtimes, 2);
    assert_eq!(docker_config.image, "image");
}

#[test]
fn recording_worker_requires_chrome_when_enabled() {
    let mut config = test_config();
    config.recording.recording_worker_bin = Some("recording-worker".into());

    let error = build_recording_worker_config(&config).unwrap_err();

    assert!(error
        .to_string()
        .contains("--recording-worker-chrome is required"));
}

#[test]
fn recording_worker_uses_inline_cert_spki() {
    let mut config = test_config();
    config.recording.recording_worker_bin = Some("recording-worker".into());
    config.recording.recording_worker_chrome = Some("chromium".into());
    config.recording.recording_worker_cert_spki = Some("inline-spki".to_string());

    let worker_config = build_recording_worker_config(&config).unwrap().unwrap();

    assert_eq!(worker_config.cert_spki.as_deref(), Some("inline-spki"));
}

#[test]
fn recording_worker_reads_cert_spki_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cert_file = temp_dir.path().join("cert-fingerprint.txt");
    std::fs::write(&cert_file, "file-spki\n").unwrap();

    let mut config = test_config();
    config.recording.recording_worker_bin = Some("recording-worker".into());
    config.recording.recording_worker_chrome = Some("chromium".into());
    config.recording.recording_worker_cert_spki_file = Some(cert_file);

    let worker_config = build_recording_worker_config(&config).unwrap().unwrap();

    assert_eq!(worker_config.cert_spki.as_deref(), Some("file-spki"));
}

#[test]
fn recording_worker_rejects_empty_cert_spki_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let cert_file = temp_dir.path().join("cert-fingerprint.txt");
    std::fs::write(&cert_file, "\n").unwrap();

    let mut config = test_config();
    config.recording.recording_worker_bin = Some("recording-worker".into());
    config.recording.recording_worker_chrome = Some("chromium".into());
    config.recording.recording_worker_cert_spki_file = Some(cert_file);

    let error = build_recording_worker_config(&config).unwrap_err();

    assert!(error
        .to_string()
        .contains("--recording-worker-cert-spki-file"));
}

#[test]
fn workflow_retention_zero_disables_cleanup_window() {
    assert!(workflow_retention_window("workflow-log-retention-secs", 0)
        .unwrap()
        .is_none());
}

#[test]
fn session_file_retention_zero_disables_cleanup_window() {
    assert!(
        session_file_retention_window("session-file-retention-secs", 0)
            .unwrap()
            .is_none()
    );
}

#[test]
fn default_owner_mode_tracks_exclusive_flag() {
    let mut config = test_config();
    assert_eq!(default_owner_mode(&config), SessionOwnerMode::Collaborative);

    config.gateway.exclusive_browser_owner = true;
    assert_eq!(
        default_owner_mode(&config),
        SessionOwnerMode::ExclusiveBrowserOwner
    );
}

#[test]
fn operational_timeouts_must_be_nonzero() {
    let mut config = test_config();
    config.gateway.readiness_check_timeout_secs = 0;
    assert!(super::validate_operational_timeouts(&config)
        .unwrap_err()
        .to_string()
        .contains("--readiness-check-timeout-secs"));

    config.gateway.readiness_check_timeout_secs = 1;
    config.gateway.shutdown_drain_timeout_secs = 0;
    assert!(super::validate_operational_timeouts(&config)
        .unwrap_err()
        .to_string()
        .contains("--shutdown-drain-timeout-secs"));

    config.gateway.shutdown_drain_timeout_secs = 2;
    config.gateway.shutdown_readiness_grace_secs = 2;
    assert!(super::validate_operational_timeouts(&config)
        .unwrap_err()
        .to_string()
        .contains("--shutdown-readiness-grace-secs"));
}

#[test]
fn browser_context_import_limits_are_validated_and_materialized() {
    let mut config = test_config();
    let service = super::build_browser_context_import_service(&config).unwrap();
    let limits = service.limits();
    assert_eq!(limits.max_archive_bytes, 536_870_912);
    assert_eq!(limits.max_profile_archive_bytes, 536_870_912);
    assert_eq!(limits.max_profile_uncompressed_bytes, 2_147_483_648);
    assert_eq!(limits.max_profile_entries, 100_000);

    config.gateway.browser_context_import_max_archive_bytes = 0;
    assert!(super::build_browser_context_import_service(&config)
        .unwrap_err()
        .to_string()
        .contains("max-archive-bytes"));

    config.gateway.browser_context_import_max_archive_bytes = 128;
    config
        .gateway
        .browser_context_import_max_profile_archive_bytes = 129;
    assert!(super::build_browser_context_import_service(&config)
        .unwrap_err()
        .to_string()
        .contains("must not exceed"));

    config
        .gateway
        .browser_context_import_max_profile_archive_bytes = 128;
    config.gateway.browser_context_import_max_concurrent = 0;
    assert!(super::build_browser_context_import_service(&config)
        .unwrap_err()
        .to_string()
        .contains("max-concurrent"));
}
