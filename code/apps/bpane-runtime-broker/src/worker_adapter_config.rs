use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context};
use bpane_runtime_contract::{ResourceLimits, SecretValue};
use serde::Deserialize;

use crate::executor::RoutedRuntimeExecutor;
use crate::{
    RecordingWorkerDockerConfig, RuntimeOperationExecutor, WorkerOidcConfig,
    WorkerRuntimeDockerAdapter, WorkerRuntimeDockerConfig, WorkflowWorkerDockerConfig,
};

const MAX_WORKER_CONFIG_BYTES: usize = 32_768;

#[derive(Debug, Clone)]
pub struct WorkerAdapterSettings {
    pub config_file: Option<PathBuf>,
    pub workflow_image: Option<String>,
    pub recording_image: Option<String>,
    pub oidc_client_secret_file: Option<PathBuf>,
}

impl WorkerAdapterSettings {
    pub fn combine_executor(
        &self,
        browser: Arc<dyn RuntimeOperationExecutor>,
        docker_api_url: Option<&str>,
        docker_timeout_secs: u64,
    ) -> anyhow::Result<Arc<dyn RuntimeOperationExecutor>> {
        let configured = self.config_file.is_some()
            || self.workflow_image.is_some()
            || self.recording_image.is_some()
            || self.oidc_client_secret_file.is_some();
        if !configured {
            return Ok(browser);
        }
        let config_file = required_path(&self.config_file, "worker configuration file")?;
        let workflow_image = required(&self.workflow_image, "workflow worker image")?;
        let recording_image = required(&self.recording_image, "recording worker image")?;
        let oidc_secret_file = required_path(
            &self.oidc_client_secret_file,
            "worker OIDC client secret file",
        )?;
        let docker_api_url = docker_api_url
            .context("Docker API URL is required when worker broker configuration is enabled")?;
        let timeout = Duration::from_secs(docker_timeout_secs);
        if timeout.is_zero() || timeout > Duration::from_secs(300) {
            bail!("worker Docker timeout must be between 1 and 300 seconds");
        }

        let document = load_document(config_file)?;
        let oidc_secret = load_secret(oidc_secret_file)?;
        let oidc = WorkerOidcConfig {
            token_url: document.oidc.token_url,
            client_id: document.oidc.client_id,
            client_secret: oidc_secret,
            scopes: document.oidc.scopes,
        };
        let workers = WorkerRuntimeDockerConfig {
            workflow: Some(workflow_config(
                document.workflow,
                workflow_image,
                oidc.clone(),
            )),
            recording: Some(recording_config(document.recording, recording_image, oidc)?),
        };
        let worker: Arc<dyn RuntimeOperationExecutor> = Arc::new(
            WorkerRuntimeDockerAdapter::connect(workers, docker_api_url, timeout)
                .map_err(|_| anyhow::anyhow!("worker Docker adapter configuration failed"))?,
        );
        Ok(Arc::new(RoutedRuntimeExecutor::new(browser, worker)))
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerConfigurationDocument {
    version: u8,
    oidc: WorkerOidcDocument,
    workflow: WorkflowWorkerDocument,
    recording: RecordingWorkerDocument,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkerOidcDocument {
    token_url: String,
    client_id: String,
    #[serde(default)]
    scopes: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowWorkerDocument {
    network: String,
    container_name_prefix: String,
    gateway_api_url: String,
    work_root: String,
    request_timeout_ms: u64,
    output_limit_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RecordingWorkerDocument {
    network: String,
    container_name_prefix: String,
    artifact_volume: String,
    chrome_executable: String,
    gateway_api_url: String,
    page_url: String,
    connect_gateway_url: String,
    output_root: String,
    cert_spki: Option<String>,
    cert_spki_file: Option<PathBuf>,
    headless: bool,
    connect_timeout_ms: u64,
    poll_interval_ms: u64,
    request_timeout_ms: u64,
}

fn load_document(path: &Path) -> anyhow::Result<WorkerConfigurationDocument> {
    let bytes = std::fs::read(path).context("failed to read worker configuration")?;
    if bytes.is_empty() || bytes.len() > MAX_WORKER_CONFIG_BYTES {
        bail!("worker configuration size is invalid");
    }
    let document: WorkerConfigurationDocument =
        serde_json::from_slice(&bytes).context("failed to parse worker configuration")?;
    if document.version != 1 {
        bail!("worker configuration version is invalid");
    }
    Ok(document)
}

fn load_secret(path: &Path) -> anyhow::Result<SecretValue> {
    let value =
        std::fs::read_to_string(path).context("failed to read worker OIDC client secret")?;
    SecretValue::new(value.trim().to_string())
        .map_err(|_| anyhow::anyhow!("worker OIDC client secret is invalid"))
}

fn workflow_config(
    document: WorkflowWorkerDocument,
    image: &str,
    oidc: WorkerOidcConfig,
) -> WorkflowWorkerDockerConfig {
    WorkflowWorkerDockerConfig {
        image: image.to_string(),
        network: document.network,
        container_name_prefix: document.container_name_prefix,
        gateway_api_url: document.gateway_api_url,
        work_root: document.work_root,
        request_timeout_ms: document.request_timeout_ms,
        output_limit_bytes: document.output_limit_bytes,
        command: worker_command(),
        seccomp_profile: "default".to_string(),
        resources: worker_resources(document.output_limit_bytes),
        oidc: Some(oidc),
    }
}

fn recording_config(
    document: RecordingWorkerDocument,
    image: &str,
    oidc: WorkerOidcConfig,
) -> anyhow::Result<RecordingWorkerDockerConfig> {
    let cert_spki = recording_cert_spki(&document)?;
    Ok(RecordingWorkerDockerConfig {
        image: image.to_string(),
        network: document.network,
        container_name_prefix: document.container_name_prefix,
        artifact_volume: document.artifact_volume,
        chrome_executable: document.chrome_executable,
        gateway_api_url: document.gateway_api_url,
        page_url: document.page_url,
        connect_gateway_url: document.connect_gateway_url,
        output_root: document.output_root,
        cert_spki,
        headless: document.headless,
        connect_timeout_ms: document.connect_timeout_ms,
        poll_interval_ms: document.poll_interval_ms,
        request_timeout_ms: document.request_timeout_ms,
        command: worker_command(),
        seccomp_profile: "default".to_string(),
        resources: worker_resources(262_144),
        oidc: Some(oidc),
    })
}

fn recording_cert_spki(document: &RecordingWorkerDocument) -> anyhow::Result<Option<String>> {
    match (&document.cert_spki, &document.cert_spki_file) {
        (Some(_), Some(_)) => bail!("recording worker certificate SPKI source is ambiguous"),
        (Some(value), None) => Ok(Some(value.clone())),
        (None, Some(path)) => {
            let value = std::fs::read_to_string(path)
                .context("failed to read recording worker certificate SPKI")?;
            let value = value.trim();
            if value.is_empty() || value.len() > 512 {
                bail!("recording worker certificate SPKI is invalid");
            }
            Ok(Some(value.to_string()))
        }
        (None, None) => Ok(None),
    }
}

fn worker_command() -> Vec<String> {
    vec![
        "/usr/local/bin/npx".to_string(),
        "tsx".to_string(),
        "src/index.ts".to_string(),
    ]
}

fn worker_resources(output_limit_bytes: u64) -> ResourceLimits {
    ResourceLimits {
        memory_bytes: 1024 * 1024 * 1024,
        cpu_millis: 2_000,
        pids: 512,
        shm_bytes: 256 * 1024 * 1024,
        timeout_secs: 3_600,
        output_limit_bytes,
    }
}

fn required<'a>(value: &'a Option<String>, name: &str) -> anyhow::Result<&'a str> {
    value
        .as_deref()
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("{name} is required when worker broker configuration is enabled"))
}

fn required_path<'a>(value: &'a Option<PathBuf>, name: &str) -> anyhow::Result<&'a Path> {
    value
        .as_deref()
        .with_context(|| format!("{name} is required when worker broker configuration is enabled"))
}

#[cfg(test)]
mod tests;
