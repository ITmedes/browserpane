use std::collections::HashMap;
use std::process::Stdio;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use uuid::Uuid;

use super::*;
use crate::session_control::{SessionOwner, SessionStore};
use crate::session_files::{SessionFileBindingMode, StoredSessionFileBinding};

const MATERIALIZE_FILE_SCRIPT: &str = r#"
set -eu
case "$BPANE_MATERIALIZE_TARGET" in
  "$BPANE_SESSION_DATA_DIR"/*) ;;
  *)
    echo "materialize target escapes BPANE_SESSION_DATA_DIR" >&2
    exit 2
    ;;
esac
target="$BPANE_MATERIALIZE_TARGET"
mode="$BPANE_MATERIALIZE_MODE"
parent="$(dirname "$target")"
mkdir -p "$parent"
tmp="${target}.tmp.$$"
trap 'rm -f "$tmp"' EXIT
cat > "$tmp"
chmod "$mode" "$tmp"
mv "$tmp" "$target"
trap - EXIT
"#;

const INITIALIZE_SESSION_DATA_SCRIPT: &str = r#"
set -eu
mkdir -p \
  "$BPANE_SESSION_DATA_DIR" \
  "$BPANE_PROFILE_DIR" \
  "$BPANE_UPLOAD_DIR" \
  "$BPANE_DOWNLOAD_DIR" \
  "$BPANE_SESSION_FILE_MOUNTS_DIR"
chown -R bpane:bpane "$BPANE_SESSION_DATA_DIR"
chmod 0770 \
  "$BPANE_SESSION_DATA_DIR" \
  "$BPANE_PROFILE_DIR" \
  "$BPANE_UPLOAD_DIR" \
  "$BPANE_DOWNLOAD_DIR" \
  "$BPANE_SESSION_FILE_MOUNTS_DIR"
"#;

const CLONE_BROWSER_CONTEXT_PROFILE_SCRIPT: &str = r#"
set -eu
mkdir -p /bpane-target
find /bpane-target -mindepth 1 -maxdepth 1 -exec rm -rf -- {} +
cp -a /bpane-source/. /bpane-target/
chown -R bpane:bpane /bpane-target
chmod 0770 /bpane-target
"#;

#[derive(Deserialize)]
struct DockerVolumeUsageEntry {
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Size")]
    size: String,
}

#[derive(Serialize)]
struct SessionFileBindingsManifest {
    format_version: u32,
    session_id: Uuid,
    owner: SessionFileBindingsManifestOwner,
    mounts_root: String,
    bindings: Vec<SessionFileBindingsManifestEntry>,
}

#[derive(Serialize)]
struct SessionFileBindingsManifestOwner {
    subject: String,
    issuer: String,
}

#[derive(Serialize)]
struct SessionFileBindingsManifestEntry {
    id: Uuid,
    source: &'static str,
    workspace_id: Uuid,
    file_id: Uuid,
    file_name: String,
    media_type: Option<String>,
    byte_count: u64,
    sha256_hex: String,
    provenance: Option<serde_json::Value>,
    mount_path: String,
    materialized_path: String,
    mode: SessionFileBindingMode,
    state: &'static str,
    labels: std::collections::HashMap<String, String>,
    created_at: chrono::DateTime<chrono::Utc>,
}

impl DockerRuntimeManager {
    pub(super) async fn initialize_session_data_volume(
        &self,
        lease: &RuntimeLease,
    ) -> Result<(), RuntimeManagerError> {
        let output = Command::new(&self.config.docker_bin)
            .args(self.docker_initialize_session_data_args(lease))
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to initialize docker session data volume for {}: {error}",
                    lease.session_id
                ))
            })?;
        if output.status.success() {
            return Ok(());
        }

        Err(RuntimeManagerError::StartupFailed(format!(
            "failed to initialize docker session data volume for {}: {}",
            lease.session_id,
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }

    pub(in crate::runtime_manager) fn docker_initialize_session_data_args(
        &self,
        lease: &RuntimeLease,
    ) -> Vec<String> {
        let mut args = vec![
            "run".to_string(),
            "--rm".to_string(),
            "--network".to_string(),
            "none".to_string(),
            "-v".to_string(),
            format!(
                "{}:{}",
                self.session_data_volume_for_session(lease.session_id),
                self.session_data_root()
            ),
        ];
        if let Some(profile_volume) = self.profile_volume_for_lease(lease) {
            args.push("-v".to_string());
            args.push(format!(
                "{}:{}",
                profile_volume,
                self.profile_dir_for_session()
            ));
        }
        args.extend([
            "-e".to_string(),
            format!("BPANE_SESSION_DATA_DIR={}", self.session_data_root()),
            "-e".to_string(),
            format!("BPANE_PROFILE_DIR={}", self.profile_dir_for_session()),
            "-e".to_string(),
            format!("BPANE_UPLOAD_DIR={}", self.upload_dir_for_session()),
            "-e".to_string(),
            format!("BPANE_DOWNLOAD_DIR={}", self.download_dir_for_session()),
            "-e".to_string(),
            format!(
                "BPANE_SESSION_FILE_MOUNTS_DIR={}",
                self.session_file_mounts_root()
            ),
            "--user".to_string(),
            "0:0".to_string(),
            "--entrypoint".to_string(),
            "/bin/sh".to_string(),
            self.config.image.clone(),
            "-ec".to_string(),
            INITIALIZE_SESSION_DATA_SCRIPT.to_string(),
        ]);
        args
    }

    pub(super) async fn remove_session_data_volume(
        &self,
        session_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        let volume = self.session_data_volume_for_session(session_id);
        self.remove_docker_volume(&volume, "session data").await
    }

    pub(super) async fn remove_browser_context_profile_volume(
        &self,
        context_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        let volume = self.browser_context_profile_volume_for_context(context_id);
        self.remove_docker_volume(&volume, "browser context profile")
            .await
    }

    pub(super) async fn clone_browser_context_profile_volume(
        &self,
        source_context_id: Uuid,
        target_context_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        let source_volume = self.browser_context_profile_volume_for_context(source_context_id);
        let target_volume = self.browser_context_profile_volume_for_context(target_context_id);
        if !self
            .docker_volume_exists(&source_volume, "browser context profile")
            .await?
        {
            return Ok(());
        }
        self.remove_browser_context_profile_volume(target_context_id)
            .await?;

        let output = Command::new(&self.config.docker_bin)
            .arg("run")
            .arg("--rm")
            .arg("-v")
            .arg(format!("{source_volume}:/bpane-source:ro"))
            .arg("-v")
            .arg(format!("{target_volume}:/bpane-target"))
            .arg("--user")
            .arg("0:0")
            .arg("--entrypoint")
            .arg("/bin/sh")
            .arg(&self.config.image)
            .arg("-ec")
            .arg(CLONE_BROWSER_CONTEXT_PROFILE_SCRIPT)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to clone docker browser context profile volume {source_volume} to {target_volume}: {error}"
                ))
            })?;
        if output.status.success() {
            return Ok(());
        }

        Err(RuntimeManagerError::StartupFailed(format!(
            "failed to clone docker browser context profile volume {source_volume} to {target_volume}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }

    pub(in crate::runtime_manager) async fn browser_context_profile_storage_bytes(
        &self,
        context_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, u64>, RuntimeManagerError> {
        if context_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let volume_by_name = context_ids
            .iter()
            .copied()
            .map(|context_id| {
                (
                    self.browser_context_profile_volume_for_context(context_id),
                    context_id,
                )
            })
            .collect::<HashMap<_, _>>();

        let output = Command::new(&self.config.docker_bin)
            .arg("system")
            .arg("df")
            .arg("-v")
            .arg("--format")
            .arg("{{json .Volumes}}")
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to inspect docker browser context profile volume usage: {error}"
                ))
            })?;
        if !output.status.success() {
            return Err(RuntimeManagerError::StartupFailed(format!(
                "failed to inspect docker browser context profile volume usage: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            )));
        }

        let entries = serde_json::from_slice::<Vec<DockerVolumeUsageEntry>>(&output.stdout)
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to parse docker browser context profile volume usage: {error}"
                ))
            })?;
        let mut storage_by_context = HashMap::new();
        for entry in entries {
            let Some(context_id) = volume_by_name.get(&entry.name).copied() else {
                continue;
            };
            let Some(bytes) = parse_docker_size_bytes(&entry.size) else {
                continue;
            };
            storage_by_context.insert(context_id, bytes);
        }
        for context_id in context_ids {
            storage_by_context.entry(*context_id).or_insert(0);
        }
        Ok(storage_by_context)
    }

    async fn remove_docker_volume(
        &self,
        volume: &str,
        label: &str,
    ) -> Result<(), RuntimeManagerError> {
        let output = Command::new(&self.config.docker_bin)
            .arg("volume")
            .arg("rm")
            .arg(volume)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to remove docker {label} volume {volume}: {error}"
                ))
            })?;
        if output.status.success() {
            return Ok(());
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_lower = stderr.to_ascii_lowercase();
        if stderr_lower.contains("no such volume") || stderr_lower.contains("not found") {
            return Ok(());
        }

        Err(RuntimeManagerError::StartupFailed(format!(
            "failed to remove docker {label} volume {volume}: {}",
            stderr.trim()
        )))
    }

    async fn docker_volume_exists(
        &self,
        volume: &str,
        label: &str,
    ) -> Result<bool, RuntimeManagerError> {
        let output = Command::new(&self.config.docker_bin)
            .arg("volume")
            .arg("inspect")
            .arg(volume)
            .output()
            .await
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to inspect docker {label} volume {volume}: {error}"
                ))
            })?;
        if output.status.success() {
            return Ok(true);
        }

        let stderr = String::from_utf8_lossy(&output.stderr);
        let stderr_lower = stderr.to_ascii_lowercase();
        if stderr_lower.contains("no such volume") || stderr_lower.contains("not found") {
            return Ok(false);
        }

        Err(RuntimeManagerError::StartupFailed(format!(
            "failed to inspect docker {label} volume {volume}: {}",
            stderr.trim()
        )))
    }

    pub(super) async fn materialize_session_file_bindings(
        &self,
        session_id: Uuid,
    ) -> Result<(), RuntimeManagerError> {
        let Some(store) = self.session_store().await else {
            return Ok(());
        };
        let bindings = store
            .list_session_file_bindings_for_session(session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?;
        if bindings.is_empty() {
            return Ok(());
        }

        let session = store
            .get_session_by_id(session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?
            .ok_or_else(|| {
                RuntimeManagerError::PersistenceFailed(format!(
                    "session {session_id} not found while materializing session file bindings"
                ))
            })?;
        let Some(workspace_file_store) = self.workspace_file_store().await else {
            let message =
                "docker runtime session file materialization requires a workspace file store"
                    .to_string();
            for binding in &bindings {
                self.fail_session_file_binding_materialization(
                    &store,
                    session_id,
                    binding.id,
                    message.clone(),
                )
                .await?;
            }
            return Err(RuntimeManagerError::StartupFailed(message));
        };

        let mut materialized_bindings = Vec::with_capacity(bindings.len());
        for binding in bindings {
            let bytes = match workspace_file_store.read(&binding.artifact_ref).await {
                Ok(bytes) => bytes,
                Err(error) => {
                    let message = format!(
                        "failed to read workspace artifact for session file binding {}: {error}",
                        binding.id
                    );
                    self.fail_session_file_binding_materialization(
                        &store,
                        session_id,
                        binding.id,
                        message.clone(),
                    )
                    .await?;
                    return Err(RuntimeManagerError::StartupFailed(message));
                }
            };

            let target_path = self.materialized_path_for_binding(&binding);
            let mode = materialization_file_mode(binding.mode);
            if let Err(error) = self
                .write_session_data_file(session_id, &target_path, mode, &bytes)
                .await
            {
                let message = error.to_string();
                self.fail_session_file_binding_materialization(
                    &store,
                    session_id,
                    binding.id,
                    message.clone(),
                )
                .await?;
                return Err(RuntimeManagerError::StartupFailed(message));
            }
            materialized_bindings.push(binding);
        }

        let manifest = self.build_session_file_manifest(
            session_id,
            &session.owner,
            materialized_bindings.as_slice(),
        )?;
        if let Err(error) = self
            .write_session_data_file(
                session_id,
                &self.session_file_manifest_path(),
                "0444",
                &manifest,
            )
            .await
        {
            let message = error.to_string();
            for binding in &materialized_bindings {
                self.fail_session_file_binding_materialization(
                    &store,
                    session_id,
                    binding.id,
                    message.clone(),
                )
                .await?;
            }
            return Err(RuntimeManagerError::StartupFailed(message));
        }

        for binding in materialized_bindings {
            store
                .mark_session_file_binding_materialized(session_id, binding.id)
                .await
                .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?;
        }

        Ok(())
    }

    pub(in crate::runtime_manager) fn materialized_path_for_binding(
        &self,
        binding: &StoredSessionFileBinding,
    ) -> String {
        format!("{}/{}", self.session_file_mounts_root(), binding.mount_path)
    }

    pub(in crate::runtime_manager) fn docker_materialize_file_args(
        &self,
        session_id: Uuid,
        target_path: &str,
        mode: &str,
    ) -> Vec<String> {
        vec![
            "run".to_string(),
            "--rm".to_string(),
            "-i".to_string(),
            "--network".to_string(),
            "none".to_string(),
            "-v".to_string(),
            format!(
                "{}:{}",
                self.session_data_volume_for_session(session_id),
                self.session_data_root()
            ),
            "-e".to_string(),
            format!("BPANE_SESSION_DATA_DIR={}", self.session_data_root()),
            "-e".to_string(),
            format!("BPANE_MATERIALIZE_TARGET={target_path}"),
            "-e".to_string(),
            format!("BPANE_MATERIALIZE_MODE={mode}"),
            "--entrypoint".to_string(),
            "/bin/sh".to_string(),
            self.config.image.clone(),
            "-ec".to_string(),
            MATERIALIZE_FILE_SCRIPT.to_string(),
        ]
    }

    pub(in crate::runtime_manager) fn build_session_file_manifest(
        &self,
        session_id: Uuid,
        owner: &SessionOwner,
        bindings: &[StoredSessionFileBinding],
    ) -> Result<Vec<u8>, RuntimeManagerError> {
        let manifest = SessionFileBindingsManifest {
            format_version: 1,
            session_id,
            owner: SessionFileBindingsManifestOwner {
                subject: owner.subject.clone(),
                issuer: owner.issuer.clone(),
            },
            mounts_root: self.session_file_mounts_root(),
            bindings: bindings
                .iter()
                .map(|binding| SessionFileBindingsManifestEntry {
                    id: binding.id,
                    source: "workspace",
                    workspace_id: binding.workspace_id,
                    file_id: binding.file_id,
                    file_name: binding.file_name.clone(),
                    media_type: binding.media_type.clone(),
                    byte_count: binding.byte_count,
                    sha256_hex: binding.sha256_hex.clone(),
                    provenance: binding.provenance.clone(),
                    mount_path: binding.mount_path.clone(),
                    materialized_path: self.materialized_path_for_binding(binding),
                    mode: binding.mode,
                    state: "materialized",
                    labels: binding.labels.clone(),
                    created_at: binding.created_at,
                })
                .collect(),
        };
        serde_json::to_vec_pretty(&manifest).map_err(|error| {
            RuntimeManagerError::StartupFailed(format!(
                "failed to encode session file binding manifest: {error}"
            ))
        })
    }

    async fn write_session_data_file(
        &self,
        session_id: Uuid,
        target_path: &str,
        mode: &str,
        bytes: &[u8],
    ) -> Result<(), RuntimeManagerError> {
        let mut child = Command::new(&self.config.docker_bin)
            .args(self.docker_materialize_file_args(session_id, target_path, mode))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| {
                RuntimeManagerError::StartupFailed(format!(
                    "failed to start docker session data writer for {target_path}: {error}"
                ))
            })?;
        let Some(mut stdin) = child.stdin.take() else {
            let _ = child.kill().await;
            return Err(RuntimeManagerError::StartupFailed(format!(
                "docker session data writer for {target_path} did not expose stdin"
            )));
        };
        if let Err(error) = stdin.write_all(bytes).await {
            let _ = child.kill().await;
            return Err(RuntimeManagerError::StartupFailed(format!(
                "failed to stream session data file {target_path} into docker writer: {error}"
            )));
        }
        drop(stdin);

        let output = child.wait_with_output().await.map_err(|error| {
            RuntimeManagerError::StartupFailed(format!(
                "failed to wait for docker session data writer for {target_path}: {error}"
            ))
        })?;
        if output.status.success() {
            return Ok(());
        }

        Err(RuntimeManagerError::StartupFailed(format!(
            "failed to write session data file {target_path}: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        )))
    }

    async fn fail_session_file_binding_materialization(
        &self,
        store: &SessionStore,
        session_id: Uuid,
        binding_id: Uuid,
        error: String,
    ) -> Result<(), RuntimeManagerError> {
        store
            .fail_session_file_binding_materialization(session_id, binding_id, error)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?;
        Ok(())
    }
}

pub(in crate::runtime_manager) fn parse_docker_size_bytes(value: &str) -> Option<u64> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("n/a") {
        return None;
    }
    let split_at = trimmed
        .char_indices()
        .find(|(_, character)| !(character.is_ascii_digit() || *character == '.'))
        .map(|(index, _)| index)
        .unwrap_or(trimmed.len());
    let number = trimmed[..split_at].trim().parse::<f64>().ok()?;
    if !number.is_finite() || number < 0.0 {
        return None;
    }
    let unit = trimmed[split_at..].trim().to_ascii_lowercase();
    let multiplier = match unit.as_str() {
        "" | "b" => 1.0,
        "kb" => 1_000.0,
        "mb" => 1_000_000.0,
        "gb" => 1_000_000_000.0,
        "tb" => 1_000_000_000_000.0,
        "kib" => 1_024.0,
        "mib" => 1_048_576.0,
        "gib" => 1_073_741_824.0,
        "tib" => 1_099_511_627_776.0,
        _ => return None,
    };
    Some((number * multiplier).round() as u64)
}

fn materialization_file_mode(mode: SessionFileBindingMode) -> &'static str {
    match mode {
        SessionFileBindingMode::ReadOnly => "0444",
        SessionFileBindingMode::ReadWrite | SessionFileBindingMode::ScratchOutput => "0666",
    }
}
