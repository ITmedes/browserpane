use std::collections::{BTreeMap, BTreeSet, HashMap};

use bollard::models::{ContainerCreateBody, HostConfig, HostConfigLogConfig, Mount, MountType};
use bpane_runtime_contract::{
    ContainerLaunchSpec, ContainerMount, MountSource, RuntimeOperationKind, RuntimeOperationResult,
    SessionDataFileTarget, StorageHelperAction, StorageHelperRequest,
};
use sha2::{Digest, Sha256};

use crate::StorageExecutionOutput;

use super::StorageRuntimeDockerConfig;

pub(super) struct MaterializedStorageHelper {
    pub(super) container_name: String,
    pub(super) policy_spec: ContainerLaunchSpec,
    pub(super) container: ContainerCreateBody,
    pub(super) output_limit: usize,
    pub(super) action: StorageHelperAction,
    pub(super) input_volume: Option<String>,
}

impl MaterializedStorageHelper {
    pub(super) fn new(
        config: &StorageRuntimeDockerConfig,
        request_id: uuid::Uuid,
        request: &StorageHelperRequest,
    ) -> Result<Self, &'static str> {
        let action = request.action;
        let mut inputs = storage_inputs(config, request_id, request)?;
        let expects_input = action.accepts_input_payload();
        let input_volume = expects_input.then(|| config.input_volume(request_id));
        if let Some(volume) = input_volume.as_ref() {
            inputs
                .mounts
                .push(mount(volume.clone(), config.input_mount_root(), false));
        }
        let container_name = config.container_name(request_id);
        let labels = storage_labels(request_id, inputs.target_id, action);
        let policy_spec = ContainerLaunchSpec {
            operation_kind: RuntimeOperationKind::StorageHelper,
            resource_id: request_id,
            owned_volume_ids: inputs.owned_volume_ids,
            image: config.image.clone(),
            container_name: container_name.clone(),
            network: None,
            mounts: inputs.mounts.iter().map(policy_mount).collect(),
            environment_keys: inputs
                .environment
                .iter()
                .filter_map(|value| value.split_once('=').map(|(key, _)| key.to_string()))
                .collect(),
            labels: labels.clone(),
            entrypoint: config.command(),
            security: config.policy_security(),
            resources: config.resources.clone(),
        };
        if expects_input {
            let payload_bytes = request
                .declared_payload_bytes
                .ok_or("storage input byte count is required")?;
            inputs.environment.push(pair(
                "BPANE_STORAGE_INPUT_BYTES",
                &payload_bytes.to_string(),
            )?);
        }
        let container = ContainerCreateBody {
            image: Some(config.image.clone()),
            env: Some(inputs.environment),
            entrypoint: Some(vec!["/bin/sh".to_string()]),
            cmd: Some(vec![
                "-ec".to_string(),
                super::STORAGE_HELPER_SCRIPT.to_string(),
            ]),
            labels: Some(labels.into_iter().collect::<HashMap<_, _>>()),
            user: Some("bpane:bpane".to_string()),
            attach_stdin: Some(false),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            open_stdin: Some(false),
            network_disabled: Some(true),
            host_config: Some(host_config(config, inputs.mounts)),
            ..Default::default()
        };
        let output_limit = if action.produces_output_payload() {
            config.max_payload_bytes
        } else {
            usize::try_from(config.resources.output_limit_bytes)
                .unwrap_or(usize::MAX)
                .min(config.max_payload_bytes)
        };
        Ok(Self {
            container_name,
            policy_spec,
            container,
            output_limit,
            action,
            input_volume,
        })
    }

    pub(super) fn result(
        action: StorageHelperAction,
        output: Vec<u8>,
    ) -> Result<StorageExecutionOutput, &'static str> {
        match action {
            StorageHelperAction::ExportBrowserContext => {
                let payload_bytes =
                    u64::try_from(output.len()).map_err(|_| "storage output is too large")?;
                if output.is_empty() {
                    return Err("storage export is empty");
                }
                let sha256_hex = hex::encode(Sha256::digest(&output));
                Ok(StorageExecutionOutput {
                    result: RuntimeOperationResult::StoragePayload {
                        payload_bytes,
                        sha256_hex,
                    },
                    payload: Some(output),
                })
            }
            StorageHelperAction::MeasureBrowserContext => {
                let value = std::str::from_utf8(&output)
                    .map_err(|_| "storage usage output is invalid")?
                    .trim()
                    .parse::<u64>()
                    .map_err(|_| "storage usage output is invalid")?;
                Ok(StorageExecutionOutput {
                    result: RuntimeOperationResult::StorageUsage {
                        storage_bytes: value,
                    },
                    payload: None,
                })
            }
            _ if output.is_empty() => Ok(StorageExecutionOutput {
                result: RuntimeOperationResult::Completed {
                    exit_code: Some(0),
                    omitted_output_bytes: 0,
                },
                payload: None,
            }),
            _ => Err("storage helper produced unexpected output"),
        }
    }
}

struct StorageInputs {
    target_id: uuid::Uuid,
    owned_volume_ids: BTreeSet<uuid::Uuid>,
    mounts: Vec<Mount>,
    environment: Vec<String>,
}

fn storage_inputs(
    config: &StorageRuntimeDockerConfig,
    request_id: uuid::Uuid,
    request: &StorageHelperRequest,
) -> Result<StorageInputs, &'static str> {
    match request.action {
        StorageHelperAction::InitializeSessionData => {
            initialize_inputs(config, request_id, request)
        }
        StorageHelperAction::MaterializeSessionFiles => file_inputs(config, request_id, request),
        StorageHelperAction::CloneBrowserContext => clone_inputs(config, request_id, request),
        StorageHelperAction::ExportBrowserContext | StorageHelperAction::MeasureBrowserContext => {
            source_inputs(config, request_id, request)
        }
        StorageHelperAction::ImportBrowserContext => import_inputs(config, request_id, request),
        StorageHelperAction::DeleteSessionData | StorageHelperAction::DeleteBrowserContext => {
            Err("delete operations do not launch helpers")
        }
    }
}

fn initialize_inputs(
    config: &StorageRuntimeDockerConfig,
    request_id: uuid::Uuid,
    request: &StorageHelperRequest,
) -> Result<StorageInputs, &'static str> {
    let session_id = request.session_id.ok_or("session id is required")?;
    let mut inputs = inputs(request_id, session_id, request.action)?;
    inputs.owned_volume_ids.insert(session_id);
    inputs.mounts.push(mount(
        config.session_data_volume(session_id),
        config.session_mount_root(),
        false,
    ));
    inputs
        .environment
        .extend(session_directory_environment(config)?);
    if let Some(context_id) = request.target_context_id {
        inputs.owned_volume_ids.insert(context_id);
        inputs.mounts.push(mount(
            config.context_volume(context_id),
            &config.profile_dir(),
            false,
        ));
    }
    Ok(inputs)
}

fn file_inputs(
    config: &StorageRuntimeDockerConfig,
    request_id: uuid::Uuid,
    request: &StorageHelperRequest,
) -> Result<StorageInputs, &'static str> {
    let session_id = request.session_id.ok_or("session id is required")?;
    let mut inputs = inputs(request_id, session_id, request.action)?;
    inputs.owned_volume_ids.insert(session_id);
    inputs.mounts.push(mount(
        config.session_data_volume(session_id),
        config.session_mount_root(),
        false,
    ));
    let (path, mode) = materialize_target(config, request)?;
    inputs
        .environment
        .push(pair("BPANE_MATERIALIZE_TARGET", &path)?);
    inputs
        .environment
        .push(pair("BPANE_MATERIALIZE_MODE", mode)?);
    Ok(inputs)
}

fn clone_inputs(
    config: &StorageRuntimeDockerConfig,
    request_id: uuid::Uuid,
    request: &StorageHelperRequest,
) -> Result<StorageInputs, &'static str> {
    let source = request
        .source_context_id
        .ok_or("source context id is required")?;
    let target = request
        .target_context_id
        .ok_or("target context id is required")?;
    let mut inputs = inputs(request_id, target, request.action)?;
    inputs.owned_volume_ids.extend([source, target]);
    inputs.mounts.extend([
        mount(
            config.context_volume(source),
            config.source_mount_root(),
            true,
        ),
        mount(
            config.context_volume(target),
            config.target_mount_root(),
            false,
        ),
    ]);
    Ok(inputs)
}

fn source_inputs(
    config: &StorageRuntimeDockerConfig,
    request_id: uuid::Uuid,
    request: &StorageHelperRequest,
) -> Result<StorageInputs, &'static str> {
    let source = request
        .source_context_id
        .ok_or("source context id is required")?;
    let mut inputs = inputs(request_id, source, request.action)?;
    inputs.owned_volume_ids.insert(source);
    inputs.mounts.push(mount(
        config.context_volume(source),
        config.source_mount_root(),
        true,
    ));
    Ok(inputs)
}

fn import_inputs(
    config: &StorageRuntimeDockerConfig,
    request_id: uuid::Uuid,
    request: &StorageHelperRequest,
) -> Result<StorageInputs, &'static str> {
    let target = request
        .target_context_id
        .ok_or("target context id is required")?;
    let mut inputs = inputs(request_id, target, request.action)?;
    inputs.owned_volume_ids.insert(target);
    inputs.mounts.push(mount(
        config.context_volume(target),
        config.target_mount_root(),
        false,
    ));
    Ok(inputs)
}

fn inputs(
    request_id: uuid::Uuid,
    target_id: uuid::Uuid,
    action: StorageHelperAction,
) -> Result<StorageInputs, &'static str> {
    Ok(StorageInputs {
        target_id,
        owned_volume_ids: BTreeSet::from([request_id]),
        mounts: Vec::new(),
        environment: vec![pair("BPANE_STORAGE_ACTION", action_name(action))?],
    })
}

fn storage_labels(
    request_id: uuid::Uuid,
    target_id: uuid::Uuid,
    action: StorageHelperAction,
) -> BTreeMap<String, String> {
    BTreeMap::from([
        (
            "browserpane.runtime.operation".to_string(),
            "storage_helper".to_string(),
        ),
        (
            "browserpane.runtime.resource_id".to_string(),
            request_id.to_string(),
        ),
        (
            "browserpane.storage.action".to_string(),
            action_name(action).to_string(),
        ),
        (
            "browserpane.storage.target_id".to_string(),
            target_id.to_string(),
        ),
    ])
}

fn session_directory_environment(
    config: &StorageRuntimeDockerConfig,
) -> Result<Vec<String>, &'static str> {
    Ok(vec![
        pair("BPANE_SESSION_DATA_DIR", config.session_mount_root())?,
        pair("BPANE_PROFILE_DIR", &config.profile_dir())?,
        pair("BPANE_UPLOAD_DIR", &config.upload_dir())?,
        pair("BPANE_DOWNLOAD_DIR", &config.download_dir())?,
        pair("BPANE_SESSION_FILE_MOUNTS_DIR", &config.mounts_dir())?,
    ])
}

fn materialize_target(
    config: &StorageRuntimeDockerConfig,
    request: &StorageHelperRequest,
) -> Result<(String, &'static str), &'static str> {
    match request
        .file_target
        .as_ref()
        .ok_or("materialize target is required")?
    {
        SessionDataFileTarget::SessionBinding {
            relative_path,
            writable,
        } => Ok((
            format!("{}/{}", config.mounts_dir(), relative_path),
            if *writable { "0666" } else { "0444" },
        )),
        SessionDataFileTarget::SessionBindingManifest => Ok((config.manifest_path(), "0444")),
        SessionDataFileTarget::EgressProxyAuthentication => Ok((config.proxy_auth_path(), "0444")),
        SessionDataFileTarget::EgressTrustedCa => Ok((config.trusted_ca_path(), "0444")),
    }
}

fn host_config(config: &StorageRuntimeDockerConfig, mounts: Vec<Mount>) -> HostConfig {
    let resources = &config.resources;
    HostConfig {
        memory: Some(resources.memory_bytes as i64),
        nano_cpus: Some(i64::from(resources.cpu_millis) * 1_000_000),
        pids_limit: Some(i64::from(resources.pids)),
        network_mode: Some("none".to_string()),
        auto_remove: Some(false),
        mounts: Some(mounts),
        privileged: Some(false),
        readonly_rootfs: Some(true),
        cap_drop: Some(vec!["ALL".to_string()]),
        security_opt: Some(vec!["no-new-privileges:true".to_string()]),
        shm_size: Some(resources.shm_bytes as i64),
        init: Some(true),
        tmpfs: Some(HashMap::from([(
            "/tmp".to_string(),
            "rw,nosuid,nodev,noexec,size=16m,mode=1777".to_string(),
        )])),
        log_config: Some(HostConfigLogConfig {
            typ: Some("local".to_string()),
            config: Some(HashMap::from([
                (
                    "max-size".to_string(),
                    format!("{}b", resources.output_limit_bytes.min(1_048_576)),
                ),
                ("max-file".to_string(), "1".to_string()),
                ("compress".to_string(), "false".to_string()),
            ])),
        }),
        ..Default::default()
    }
}

fn mount(source: String, target: &str, read_only: bool) -> Mount {
    Mount {
        source: Some(source),
        target: Some(target.to_string()),
        typ: Some(MountType::VOLUME),
        read_only: Some(read_only),
        ..Default::default()
    }
}

fn policy_mount(mount: &Mount) -> ContainerMount {
    ContainerMount {
        source: MountSource::NamedVolume(mount.source.clone().unwrap_or_default()),
        target: mount.target.clone().unwrap_or_default(),
        read_only: mount.read_only.unwrap_or(false),
    }
}

fn pair(key: &str, value: &str) -> Result<String, &'static str> {
    if value.is_empty()
        || value.len() > 16 * 1024
        || value
            .bytes()
            .any(|byte| byte == 0 || byte == b'\n' || byte == b'\r')
    {
        return Err("storage helper environment value is invalid");
    }
    Ok(format!("{key}={value}"))
}

fn action_name(action: StorageHelperAction) -> &'static str {
    match action {
        StorageHelperAction::InitializeSessionData => "initialize_session_data",
        StorageHelperAction::MaterializeSessionFiles => "materialize_session_files",
        StorageHelperAction::DeleteSessionData => "delete_session_data",
        StorageHelperAction::CloneBrowserContext => "clone_browser_context",
        StorageHelperAction::ExportBrowserContext => "export_browser_context",
        StorageHelperAction::ImportBrowserContext => "import_browser_context",
        StorageHelperAction::MeasureBrowserContext => "measure_browser_context",
        StorageHelperAction::DeleteBrowserContext => "delete_browser_context",
    }
}
