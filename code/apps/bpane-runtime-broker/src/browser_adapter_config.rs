use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context};
use bpane_runtime_contract::ResourceLimits;
use clap::ValueEnum;
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    BrowserRuntimeDockerAdapter, BrowserRuntimeDockerConfig, BrowserRuntimeExtensionConfig,
    RejectingRuntimeExecutor, RuntimeOperationExecutor,
};

const MAX_EXTENSION_REGISTRY_BYTES: usize = 65_536;
const MAX_EXTENSION_REGISTRY_ENTRIES: usize = 128;
const MAX_BROWSER_ENVIRONMENT_BYTES: usize = 16_384;
const MAX_BROWSER_ENVIRONMENT_ENTRIES: usize = 64;
const TRUSTED_BROWSER_ENVIRONMENT_KEYS: [&str; 27] = [
    "BPANE_H264_MODE",
    "BPANE_H264_BITRATE",
    "BPANE_H264_MAXRATE",
    "BPANE_H264_BUFSIZE",
    "BPANE_H264_PRESET",
    "BPANE_H264_PROFILE",
    "BPANE_H264_LEVEL",
    "BPANE_H264_TUNE",
    "BPANE_H264_BFRAMES",
    "BPANE_FPS",
    "BPANE_CDP_MIN_VIDEO_WIDTH",
    "BPANE_CDP_MIN_VIDEO_HEIGHT",
    "BPANE_CDP_MIN_VIDEO_AREA_RATIO",
    "BPANE_CDP_PAUSE_VIDEOS_ON_SCROLL",
    "BPANE_CDP_SCROLL_PAUSE_WINDOW_MS",
    "BPANE_VIDEO_CLICK_ARM_MS",
    "BPANE_AUDIO_CODEC",
    "BPANE_TILE_CODEC",
    "RUST_LOG",
    "BPANE_URL",
    "BPANE_DPI",
    "GDK_SCALE",
    "GDK_DPI_SCALE",
    "BPANE_PROFILE_ROOT",
    "BPANE_SESSION_ID",
    "BPANE_CDP_PROXY_PORT",
    "BPANE_CHROMIUM_POLICY_FILE",
];

/// Runtime operation executor selected by trusted broker configuration.
#[derive(Debug, Clone, Copy, Default, Eq, PartialEq, ValueEnum)]
pub enum RuntimeExecutorMode {
    /// Deny every runtime operation.
    #[default]
    Rejecting,
    /// Enable only the policy-validating browser Docker adapter.
    DockerBrowser,
}

/// Trusted browser adapter settings parsed by the broker process.
#[derive(Debug, Clone)]
pub struct BrowserAdapterSettings {
    pub mode: RuntimeExecutorMode,
    pub docker_api_url: Option<String>,
    pub image: Option<String>,
    pub network: Option<String>,
    pub socket_volume: Option<String>,
    pub session_data_volume_prefix: String,
    pub browser_context_volume_prefix: String,
    pub container_name_prefix: String,
    pub socket_mount_root: String,
    pub socket_path_root: String,
    pub session_data_root: String,
    pub extension_registry_file: Option<PathBuf>,
    pub browser_environment_file: Option<PathBuf>,
    pub docker_timeout_secs: u64,
}

impl BrowserAdapterSettings {
    /// Builds the selected executor after validating every trusted setting.
    pub fn build_executor(&self) -> anyhow::Result<Arc<dyn RuntimeOperationExecutor>> {
        match self.mode {
            RuntimeExecutorMode::Rejecting => {
                if self.docker_api_url.is_some()
                    || self.image.is_some()
                    || self.network.is_some()
                    || self.socket_volume.is_some()
                    || self.extension_registry_file.is_some()
                    || self.browser_environment_file.is_some()
                {
                    bail!("browser adapter settings require docker-browser executor mode");
                }
                Ok(Arc::new(RejectingRuntimeExecutor))
            }
            RuntimeExecutorMode::DockerBrowser => {
                let docker_api_url = required(&self.docker_api_url, "Docker API URL")?;
                let config = BrowserRuntimeDockerConfig {
                    image: required(&self.image, "browser image")?.to_string(),
                    network: required(&self.network, "browser network")?.to_string(),
                    socket_volume: required(&self.socket_volume, "browser socket volume")?
                        .to_string(),
                    session_data_volume_prefix: self.session_data_volume_prefix.clone(),
                    browser_context_volume_prefix: self.browser_context_volume_prefix.clone(),
                    container_name_prefix: self.container_name_prefix.clone(),
                    socket_mount_root: self.socket_mount_root.clone(),
                    socket_path_root: self.socket_path_root.clone(),
                    session_data_root: self.session_data_root.clone(),
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
                    extensions: load_extension_registry(self.extension_registry_file.as_deref())?,
                    base_environment: load_browser_environment(
                        self.browser_environment_file.as_deref(),
                    )?,
                };
                let timeout = Duration::from_secs(self.docker_timeout_secs);
                if timeout.is_zero() || timeout > Duration::from_secs(300) {
                    bail!("browser Docker timeout must be between 1 and 300 seconds");
                }
                Ok(Arc::new(
                    BrowserRuntimeDockerAdapter::connect(config, docker_api_url, timeout).map_err(
                        |_| anyhow::anyhow!("browser Docker adapter configuration failed"),
                    )?,
                ))
            }
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionRegistryDocument {
    version: u8,
    extensions: Vec<ExtensionRegistryEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionRegistryEntry {
    extension_version_id: Uuid,
    install_path: String,
}

fn load_extension_registry(
    path: Option<&Path>,
) -> anyhow::Result<BTreeMap<Uuid, BrowserRuntimeExtensionConfig>> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let bytes = std::fs::read(path).context("failed to read browser extension registry")?;
    if bytes.is_empty() || bytes.len() > MAX_EXTENSION_REGISTRY_BYTES {
        bail!("browser extension registry size is invalid");
    }
    let document: ExtensionRegistryDocument =
        serde_json::from_slice(&bytes).context("failed to parse browser extension registry")?;
    if document.version != 1 || document.extensions.len() > MAX_EXTENSION_REGISTRY_ENTRIES {
        bail!("browser extension registry contract is invalid");
    }
    let mut extensions = BTreeMap::new();
    for entry in document.extensions {
        if entry.extension_version_id.is_nil()
            || extensions
                .insert(
                    entry.extension_version_id,
                    BrowserRuntimeExtensionConfig {
                        extension_version_id: entry.extension_version_id,
                        install_path: entry.install_path,
                    },
                )
                .is_some()
        {
            bail!("browser extension registry identity is invalid");
        }
    }
    Ok(extensions)
}

fn load_browser_environment(path: Option<&Path>) -> anyhow::Result<BTreeMap<String, String>> {
    let Some(path) = path else {
        return Ok(BTreeMap::new());
    };
    let bytes = std::fs::read(path).context("failed to read browser environment file")?;
    if bytes.is_empty() || bytes.len() > MAX_BROWSER_ENVIRONMENT_BYTES {
        bail!("browser environment file size is invalid");
    }
    let content = std::str::from_utf8(&bytes).context("browser environment file is not UTF-8")?;
    let mut environment = BTreeMap::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .context("browser environment entry is malformed")?;
        let key = key.trim();
        if !TRUSTED_BROWSER_ENVIRONMENT_KEYS.contains(&key)
            || value.len() > 2_048
            || value
                .bytes()
                .any(|byte| byte == 0 || byte.is_ascii_control())
            || environment.contains_key(key)
        {
            bail!("browser environment entry is invalid");
        }
        if key != "BPANE_SESSION_ID" {
            environment.insert(key.to_string(), value.to_string());
        }
        if environment.len() > MAX_BROWSER_ENVIRONMENT_ENTRIES {
            bail!("browser environment contains too many entries");
        }
    }
    Ok(environment)
}

fn required<'a>(value: &'a Option<String>, name: &str) -> anyhow::Result<&'a str> {
    value
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("{name} is required for docker-browser executor mode"))
}

#[cfg(test)]
mod tests;
