use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use bollard::models::ContainerCreateBody;
use bpane_runtime_contract::{
    BrokerApiVersion, IdempotencyKey, ResourceLimits, RuntimeOperation, RuntimeOperationRequest,
    RuntimeOperationResult, SessionDataFileTarget, StorageHelperAction, StorageHelperRequest,
};
use uuid::Uuid;

use super::backend::{StorageDockerApi, StorageDockerError};
use super::*;

#[derive(Default)]
struct MockStorageDockerApi {
    volumes: Mutex<BTreeSet<String>>,
    removed: Mutex<Vec<String>>,
    launches: Mutex<Vec<CapturedLaunch>>,
    fail_launch: AtomicBool,
}

struct CapturedLaunch {
    name: String,
    body: ContainerCreateBody,
    input: Option<Vec<u8>>,
    output_limit: usize,
}

#[async_trait]
impl StorageDockerApi for MockStorageDockerApi {
    async fn ping(&self) -> Result<(), StorageDockerError> {
        Ok(())
    }

    async fn volume_exists(&self, name: &str) -> Result<bool, StorageDockerError> {
        Ok(self.volumes.lock().unwrap().contains(name))
    }

    async fn remove_volume(&self, name: &str) -> Result<(), StorageDockerError> {
        if self.volumes.lock().unwrap().remove(name) {
            self.removed.lock().unwrap().push(name.to_string());
            Ok(())
        } else {
            Err(StorageDockerError::NotFound)
        }
    }

    async fn run_helper(
        &self,
        name: String,
        body: ContainerCreateBody,
        input: Option<Vec<u8>>,
        output_limit: usize,
    ) -> Result<Vec<u8>, StorageDockerError> {
        let action = body
            .env
            .as_ref()
            .and_then(|entries| {
                entries
                    .iter()
                    .find_map(|entry| entry.strip_prefix("BPANE_STORAGE_ACTION="))
            })
            .unwrap_or_default()
            .to_string();
        self.launches.lock().unwrap().push(CapturedLaunch {
            name,
            body,
            input,
            output_limit,
        });
        if self.fail_launch.load(Ordering::SeqCst) {
            if let Some(mounts) = self
                .launches
                .lock()
                .unwrap()
                .last()
                .and_then(|launch| launch.body.host_config.as_ref())
                .and_then(|host| host.mounts.as_ref())
            {
                for volume in mounts.iter().filter_map(|mount| {
                    matches!(
                        mount.target.as_deref(),
                        Some("/run/bpane/storage-helper/target") | Some(STORAGE_INPUT_MOUNT_ROOT)
                    )
                    .then(|| mount.source.clone())
                    .flatten()
                }) {
                    self.volumes.lock().unwrap().insert(volume);
                }
            }
            return Err(StorageDockerError::Failed);
        }
        Ok(match action.as_str() {
            "export_browser_context" => b"context-archive".to_vec(),
            "measure_browser_context" => b"42\n".to_vec(),
            _ => Vec::new(),
        })
    }
}

fn config() -> StorageRuntimeDockerConfig {
    StorageRuntimeDockerConfig {
        image: "registry.example/browser@sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string(),
        session_data_volume_prefix: "bpane-session-data".to_string(),
        browser_context_volume_prefix: "bpane-browser-context".to_string(),
        container_name_prefix: "bpane-storage-helper".to_string(),
        seccomp_profile: "default".to_string(),
        resources: ResourceLimits {
            memory_bytes: 256 * 1024 * 1024,
            cpu_millis: 1_000,
            pids: 64,
            shm_bytes: 16 * 1024 * 1024,
            timeout_secs: 30,
            output_limit_bytes: 1024 * 1024,
        },
        max_payload_bytes: 1024 * 1024,
        max_archive_entries: 100,
        max_archive_path_bytes: 256,
        max_archive_uncompressed_bytes: 1024 * 1024,
    }
}

fn operation(storage: StorageHelperRequest) -> RuntimeOperationRequest {
    RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::now_v7(),
        idempotency_key: IdempotencyKey::new(format!("storage:{}", Uuid::now_v7())).unwrap(),
        operation: RuntimeOperation::RunStorageHelper(storage),
    }
}

fn storage(action: StorageHelperAction) -> StorageHelperRequest {
    StorageHelperRequest {
        action,
        session_id: None,
        source_context_id: None,
        target_context_id: None,
        file_target: None,
        declared_payload_bytes: None,
    }
}

#[cfg(unix)]
#[test]
fn fixed_helper_script_is_valid_posix_shell_syntax() {
    let status = std::process::Command::new("/bin/sh")
        .args(["-n", "-c", STORAGE_HELPER_SCRIPT])
        .status()
        .unwrap();
    assert!(status.success());
}

#[test]
fn fixed_helper_script_consumes_and_verifies_the_staged_payload() {
    assert!(STORAGE_HELPER_SCRIPT.contains("/run/bpane/storage-helper/input/payload"));
    assert!(STORAGE_HELPER_SCRIPT.contains("wc -c"));
    assert!(!STORAGE_HELPER_SCRIPT.contains("dd bs=1"));
}

#[tokio::test]
async fn initializes_session_with_only_owned_fixed_mounts() {
    let backend = Arc::new(MockStorageDockerApi::default());
    let context_id = Uuid::now_v7();
    backend
        .volumes
        .lock()
        .unwrap()
        .insert(config().context_volume(context_id));
    let adapter = StorageRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let session_id = Uuid::now_v7();
    let mut request = storage(StorageHelperAction::InitializeSessionData);
    request.session_id = Some(session_id);
    request.target_context_id = Some(context_id);
    let result = adapter
        .execute_storage(&operation(request), None)
        .await
        .unwrap();
    assert!(matches!(
        result.result,
        RuntimeOperationResult::Completed { .. }
    ));

    let launches = backend.launches.lock().unwrap();
    let launch = launches.first().unwrap();
    let body = &launch.body;
    assert!(launch.name.starts_with("bpane-storage-helper-"));
    assert_eq!(launch.output_limit, 1024 * 1024);
    assert_eq!(body.network_disabled, Some(true));
    assert_eq!(body.user.as_deref(), Some("bpane:bpane"));
    assert_eq!(
        body.host_config.as_ref().unwrap().network_mode.as_deref(),
        Some("none")
    );
    assert_eq!(
        body.host_config.as_ref().unwrap().readonly_rootfs,
        Some(true)
    );
    assert_eq!(
        body.host_config
            .as_ref()
            .unwrap()
            .cap_drop
            .as_ref()
            .unwrap(),
        &["ALL".to_string()]
    );
    let mounts = body.host_config.as_ref().unwrap().mounts.as_ref().unwrap();
    assert_eq!(mounts.len(), 2);
    assert!(mounts
        .iter()
        .all(|mount| mount.typ == Some(bollard::models::MountType::VOLUME)));
    assert!(mounts
        .iter()
        .all(|mount| mount.target.as_deref() != Some("/")));
}

#[tokio::test]
async fn materializes_only_typed_session_destination() {
    let backend = Arc::new(MockStorageDockerApi::default());
    let adapter = StorageRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let bytes = b"workspace data";
    let mut request = storage(StorageHelperAction::MaterializeSessionFiles);
    request.session_id = Some(Uuid::now_v7());
    request.file_target = Some(SessionDataFileTarget::SessionBinding {
        relative_path: "in/report.txt".to_string(),
        writable: false,
    });
    request.declared_payload_bytes = Some(bytes.len() as u64);
    adapter
        .execute_storage(&operation(request), Some(bytes))
        .await
        .unwrap();
    let launches = backend.launches.lock().unwrap();
    let launch = launches.first().unwrap();
    let body = &launch.body;
    assert_eq!(launch.input.as_deref(), Some(bytes.as_slice()));
    let environment = body.env.as_ref().unwrap();
    assert!(environment.iter().any(|entry| {
        entry == "BPANE_MATERIALIZE_TARGET=/run/bpane/storage-helper/session/mounts/in/report.txt"
    }));
    assert!(environment
        .iter()
        .any(|entry| entry == "BPANE_MATERIALIZE_MODE=0444"));
    assert!(environment
        .iter()
        .any(|entry| entry == "BPANE_STORAGE_INPUT_BYTES=14"));
    let mounts = body.host_config.as_ref().unwrap().mounts.as_ref().unwrap();
    assert!(mounts.iter().any(|mount| {
        mount.target.as_deref() == Some(STORAGE_INPUT_MOUNT_ROOT) && mount.read_only == Some(false)
    }));
    assert!(mounts.iter().any(|mount| {
        mount
            .source
            .as_deref()
            .is_some_and(|source| source.starts_with("bpane-storage-helper-input-"))
    }));
    assert_eq!(body.attach_stdin, Some(false));
    assert_eq!(body.open_stdin, Some(false));
}

#[tokio::test]
async fn absent_source_does_not_create_context_volume() {
    let backend = Arc::new(MockStorageDockerApi::default());
    let adapter = StorageRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let mut request = storage(StorageHelperAction::ExportBrowserContext);
    request.source_context_id = Some(Uuid::now_v7());
    let result = adapter
        .execute_storage(&operation(request), None)
        .await
        .unwrap();
    assert_eq!(result.result, RuntimeOperationResult::Absent);
    assert!(backend.launches.lock().unwrap().is_empty());
}

#[tokio::test]
async fn exports_payload_with_exact_digest() {
    let backend = Arc::new(MockStorageDockerApi::default());
    let context_id = Uuid::now_v7();
    backend
        .volumes
        .lock()
        .unwrap()
        .insert(config().context_volume(context_id));
    let adapter = StorageRuntimeDockerAdapter::with_backend(config(), backend).unwrap();
    let mut request = storage(StorageHelperAction::ExportBrowserContext);
    request.source_context_id = Some(context_id);
    let result = adapter
        .execute_storage(&operation(request), None)
        .await
        .unwrap();
    assert_eq!(result.payload.as_deref(), Some(&b"context-archive"[..]));
    assert!(matches!(
        result.result,
        RuntimeOperationResult::StoragePayload {
            payload_bytes: 15,
            ..
        }
    ));
}

#[tokio::test]
async fn removes_only_derived_owned_volume_and_is_idempotent() {
    let backend = Arc::new(MockStorageDockerApi::default());
    let session_id = Uuid::now_v7();
    backend
        .volumes
        .lock()
        .unwrap()
        .insert(config().session_data_volume(session_id));
    let adapter = StorageRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let mut request = storage(StorageHelperAction::DeleteSessionData);
    request.session_id = Some(session_id);
    let request = operation(request);
    let first = adapter.execute_storage(&request, None).await.unwrap();
    let second = adapter.execute_storage(&request, None).await.unwrap();
    assert!(matches!(
        first.result,
        RuntimeOperationResult::Completed { .. }
    ));
    assert_eq!(second.result, RuntimeOperationResult::Absent);
    assert_eq!(
        backend.removed.lock().unwrap().as_slice(),
        &[config().session_data_volume(session_id)]
    );
}

#[tokio::test]
async fn rejects_invalid_import_before_docker_dispatch() {
    let backend = Arc::new(MockStorageDockerApi::default());
    let adapter = StorageRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let bytes = b"not an archive";
    let mut request = storage(StorageHelperAction::ImportBrowserContext);
    request.target_context_id = Some(Uuid::now_v7());
    request.declared_payload_bytes = Some(bytes.len() as u64);
    let error = adapter
        .execute_storage(&operation(request), Some(bytes))
        .await
        .unwrap_err();
    assert_eq!(error.code, ExecutionErrorCode::AdapterFailed);
    assert!(backend.launches.lock().unwrap().is_empty());
}

#[tokio::test]
async fn failed_clone_removes_partial_target_volume() {
    let backend = Arc::new(MockStorageDockerApi::default());
    let source_id = Uuid::now_v7();
    let target_id = Uuid::now_v7();
    backend
        .volumes
        .lock()
        .unwrap()
        .insert(config().context_volume(source_id));
    backend.fail_launch.store(true, Ordering::SeqCst);
    let adapter = StorageRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let mut request = storage(StorageHelperAction::CloneBrowserContext);
    request.source_context_id = Some(source_id);
    request.target_context_id = Some(target_id);
    let error = adapter
        .execute_storage(&operation(request), None)
        .await
        .unwrap_err();
    assert_eq!(error.code, ExecutionErrorCode::AdapterFailed);
    let target_volume = config().context_volume(target_id);
    assert!(!backend.volumes.lock().unwrap().contains(&target_volume));
    assert_eq!(backend.removed.lock().unwrap().as_slice(), &[target_volume]);
}

#[tokio::test]
async fn failed_materialization_removes_only_request_scoped_input() {
    let backend = Arc::new(MockStorageDockerApi::default());
    let session_id = Uuid::now_v7();
    let session_volume = config().session_data_volume(session_id);
    backend
        .volumes
        .lock()
        .unwrap()
        .insert(session_volume.clone());
    backend.fail_launch.store(true, Ordering::SeqCst);
    let adapter = StorageRuntimeDockerAdapter::with_backend(config(), backend.clone()).unwrap();
    let bytes = b"workspace data";
    let mut request = storage(StorageHelperAction::MaterializeSessionFiles);
    request.session_id = Some(session_id);
    request.file_target = Some(SessionDataFileTarget::SessionBindingManifest);
    request.declared_payload_bytes = Some(bytes.len() as u64);
    let operation = operation(request);
    let request_id = operation.request_id;
    let error = adapter
        .execute_storage(&operation, Some(bytes))
        .await
        .unwrap_err();
    assert_eq!(error.code, ExecutionErrorCode::AdapterFailed);
    let input_volume = config().input_volume(request_id);
    let volumes = backend.volumes.lock().unwrap();
    assert!(volumes.contains(&session_volume));
    assert!(!volumes.contains(&input_volume));
    drop(volumes);
    assert_eq!(backend.removed.lock().unwrap().as_slice(), &[input_volume]);
}
