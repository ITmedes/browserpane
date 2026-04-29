use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue, CONTENT_TYPE};
use reqwest::{Method, StatusCode};
use serde_json::{json, Value};
use tempfile::{Builder, TempDir};
use tokio::sync::Mutex;
use tokio::time::sleep;
use uuid::Uuid;

const DEFAULT_API_BASE_URL: &str = "http://localhost:8932";
const DEFAULT_TOKEN_URL: &str =
    "http://localhost:8091/realms/browserpane-dev/protocol/openid-connect/token";
const DEFAULT_OIDC_CLIENT_ID: &str = "bpane-mcp-bridge";
const DEFAULT_OIDC_CLIENT_SECRET: &str = "bpane-mcp-bridge-secret";
const DEFAULT_CONTAINER_WORKSPACE_ROOT: &str = "/workspace";
const DEFAULT_MCP_BRIDGE_BASE_URL: &str = "http://localhost:8931";

pub fn suite_lock() -> &'static Mutex<()> {
    static SUITE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    SUITE_LOCK.get_or_init(|| Mutex::new(()))
}

#[derive(Clone)]
pub struct ComposeHarness {
    client: reqwest::Client,
    api_base_url: String,
    mcp_bridge_base_url: String,
    access_token: Arc<String>,
    repo_root: Arc<PathBuf>,
    container_workspace_root: Arc<String>,
}

pub struct LocalWorkflowRepo {
    _temp_dir: TempDir,
    pub repository_url: String,
    pub commit: String,
}

pub struct ComposeVisibleFile {
    _temp_dir: TempDir,
    pub container_path: String,
}

#[allow(dead_code)]
pub struct ComposeServiceRestoreGuard {
    repo_root: PathBuf,
    services: Vec<String>,
}

pub struct JsonOutcome {
    pub status: StatusCode,
    pub body: Value,
}

impl ComposeHarness {
    pub async fn connect() -> Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .context("failed to build reqwest client")?;
        let api_base_url = std::env::var("BPANE_GATEWAY_E2E_API_URL")
            .unwrap_or_else(|_| DEFAULT_API_BASE_URL.to_string());
        let token_url = std::env::var("BPANE_GATEWAY_E2E_TOKEN_URL")
            .unwrap_or_else(|_| DEFAULT_TOKEN_URL.into());
        let client_id = std::env::var("BPANE_GATEWAY_E2E_CLIENT_ID")
            .unwrap_or_else(|_| DEFAULT_OIDC_CLIENT_ID.to_string());
        let client_secret = std::env::var("BPANE_GATEWAY_E2E_CLIENT_SECRET")
            .unwrap_or_else(|_| DEFAULT_OIDC_CLIENT_SECRET.to_string());
        let mcp_bridge_base_url = std::env::var("BPANE_GATEWAY_E2E_MCP_BRIDGE_URL")
            .unwrap_or_else(|_| DEFAULT_MCP_BRIDGE_BASE_URL.to_string());
        let repo_root = repository_root()?;
        let container_workspace_root = std::env::var("BPANE_GATEWAY_E2E_CONTAINER_WORKSPACE_ROOT")
            .unwrap_or_else(|_| DEFAULT_CONTAINER_WORKSPACE_ROOT.to_string());

        let access_token =
            fetch_client_credentials_token(&client, &token_url, &client_id, &client_secret)
                .await
                .context("failed to fetch OIDC client credentials token for compose e2e suite")?;

        Ok(Self {
            client,
            api_base_url,
            mcp_bridge_base_url,
            access_token: Arc::new(access_token),
            repo_root: Arc::new(repo_root),
            container_workspace_root: Arc::new(container_workspace_root),
        })
    }

    pub fn bearer_token(&self) -> &str {
        self.access_token.as_str()
    }

    pub fn api_url(&self, path: &str) -> String {
        format!("{}{}", self.api_base_url, path)
    }

    pub fn mcp_bridge_url(&self, path: &str) -> String {
        format!("{}{}", self.mcp_bridge_base_url, path)
    }

    pub async fn get_json(&self, path: &str) -> Result<Value> {
        self.send_json(Method::GET, path, None::<Value>, None).await
    }

    pub async fn get_json_outcome(&self, path: &str) -> Result<JsonOutcome> {
        self.send_json_outcome(Method::GET, path, None::<Value>, None)
            .await
    }

    pub async fn get_json_with_headers(&self, path: &str, headers: HeaderMap) -> Result<Value> {
        self.send_json(Method::GET, path, None::<Value>, Some(headers))
            .await
    }

    pub async fn delete_json(&self, path: &str) -> Result<Value> {
        self.send_json(Method::DELETE, path, None::<Value>, None)
            .await
    }

    pub async fn delete_json_outcome(&self, path: &str) -> Result<JsonOutcome> {
        self.send_json_outcome(Method::DELETE, path, None::<Value>, None)
            .await
    }

    pub async fn post_json(&self, path: &str, body: Value) -> Result<Value> {
        self.send_json(Method::POST, path, Some(body), None).await
    }

    pub async fn post_json_outcome(&self, path: &str, body: Value) -> Result<JsonOutcome> {
        self.send_json_outcome(Method::POST, path, Some(body), None)
            .await
    }

    pub async fn get_bridge_json(&self, path: &str) -> Result<Value> {
        self.send_bridge_json(Method::GET, path, None::<Value>)
            .await
    }

    pub async fn post_bridge_json(&self, path: &str, body: Value) -> Result<Value> {
        self.send_bridge_json(Method::POST, path, Some(body)).await
    }

    pub async fn put_bridge_json(&self, path: &str, body: Value) -> Result<Value> {
        self.send_bridge_json(Method::PUT, path, Some(body)).await
    }

    pub async fn delete_bridge_json(&self, path: &str) -> Result<Value> {
        self.send_bridge_json(Method::DELETE, path, None::<Value>)
            .await
    }

    pub async fn post_json_with_headers(
        &self,
        path: &str,
        body: Value,
        headers: HeaderMap,
    ) -> Result<Value> {
        self.send_json(Method::POST, path, Some(body), Some(headers))
            .await
    }

    pub async fn post_bytes(
        &self,
        path: &str,
        bytes: Vec<u8>,
        content_type: &str,
        extra_headers: &[(&str, &str)],
    ) -> Result<Value> {
        let mut headers = HeaderMap::new();
        headers.insert(
            CONTENT_TYPE,
            HeaderValue::from_str(content_type).context("invalid content-type header")?,
        );
        for (name, value) in extra_headers {
            headers.insert(
                HeaderName::from_bytes(name.as_bytes())
                    .with_context(|| format!("invalid header name {name}"))?,
                HeaderValue::from_str(value)
                    .with_context(|| format!("invalid header value for {name}"))?,
            );
        }
        self.send_bytes(Method::POST, path, bytes, headers).await
    }

    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>> {
        self.send_for_bytes(Method::GET, path, None).await
    }

    pub async fn get_bytes_with_automation_token(
        &self,
        path: &str,
        automation_token: &str,
    ) -> Result<Vec<u8>> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-bpane-automation-access-token",
            HeaderValue::from_str(automation_token).context("invalid automation token header")?,
        );
        self.send_for_bytes(Method::GET, path, Some(headers)).await
    }

    pub async fn get_json_with_automation_token(
        &self,
        path: &str,
        automation_token: &str,
    ) -> Result<Value> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-bpane-automation-access-token",
            HeaderValue::from_str(automation_token).context("invalid automation token header")?,
        );
        self.get_json_with_headers(path, headers).await
    }

    pub async fn poll_json<F>(
        &self,
        description: &str,
        timeout: Duration,
        mut predicate: F,
        path: &str,
    ) -> Result<Value>
    where
        F: FnMut(&Value) -> bool,
    {
        let started = std::time::Instant::now();
        loop {
            let value = self.get_json(path).await?;
            if predicate(&value) {
                return Ok(value);
            }
            if started.elapsed() >= timeout {
                bail!("timed out waiting for {description}");
            }
            sleep(Duration::from_millis(500)).await;
        }
    }

    pub async fn ensure_workflow_worker_image(&self) -> Result<()> {
        let inspect = Command::new("docker")
            .args(["image", "inspect", "deploy-workflow-worker"])
            .output()
            .context("failed to run docker image inspect")?;
        if inspect.status.success() {
            return Ok(());
        }

        let build_status = Command::new("docker")
            .args([
                "compose",
                "-f",
                "deploy/compose.yml",
                "build",
                "workflow-worker",
            ])
            .current_dir(self.repo_root.as_ref())
            .status()
            .context("failed to build workflow-worker image")?;
        if !build_status.success() {
            bail!("docker compose build workflow-worker failed");
        }
        Ok(())
    }

    pub async fn cleanup_active_sessions(&self) -> Result<()> {
        self.clear_bridge_control_session().await?;
        let sessions = self.get_json("/api/v1/sessions").await?;
        let sessions = json_array(&sessions, "sessions")?;
        for session in sessions {
            if session.get("state").and_then(Value::as_str) == Some("stopped") {
                continue;
            }
            let session_id = json_id(session, "id")?;
            let _ = self.stop_session_eventually(&session_id).await?;
        }

        poll_until(
            "compose e2e active session cleanup",
            Duration::from_secs(30),
            || async {
                let sessions = self.get_json("/api/v1/sessions").await?;
                let sessions = json_array(&sessions, "sessions")?;
                if sessions
                    .iter()
                    .all(|session| session.get("state").and_then(Value::as_str) == Some("stopped"))
                {
                    return Ok(Some(()));
                }
                Ok(None)
            },
        )
        .await?;

        Ok(())
    }

    #[allow(dead_code)]
    pub async fn wait_for_gateway_api_ready(&self) -> Result<()> {
        poll_until(
            "compose gateway api readiness",
            Duration::from_secs(30),
            || {
                let harness = self.clone();
                async move {
                    match harness.get_json("/api/v1/sessions").await {
                        Ok(value) => Ok(Some(value)),
                        Err(_) => Ok(None),
                    }
                }
            },
        )
        .await?;
        Ok(())
    }

    #[allow(dead_code)]
    pub fn compose_service_restore_guard(&self, services: &[&str]) -> ComposeServiceRestoreGuard {
        ComposeServiceRestoreGuard {
            repo_root: self.repo_root().to_path_buf(),
            services: services.iter().map(|value| (*value).to_string()).collect(),
        }
    }

    #[allow(dead_code)]
    pub fn recreate_compose_services(
        &self,
        services: &[&str],
        env_overrides: &[(&str, &str)],
    ) -> Result<()> {
        let services = services
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        recreate_compose_services_blocking(self.repo_root(), &services, env_overrides)
    }

    pub async fn clear_bridge_control_session(&self) -> Result<()> {
        let _ = self.delete_bridge_json("/control-session").await;
        let _ = self.delete_json("/api/session/mcp-owner").await;
        poll_until(
            "compose e2e bridge control-session cleanup",
            Duration::from_secs(15),
            || {
                let harness = self.clone();
                async move {
                    let health = harness.get_bridge_json("/health").await?;
                    if health["control_session_id"].is_null()
                        && health["playwright_cdp_endpoint"].is_null()
                    {
                        return Ok(Some(()));
                    }
                    Ok(None)
                }
            },
        )
        .await?;
        Ok(())
    }

    pub async fn stop_session_eventually(&self, session_id: &str) -> Result<Value> {
        poll_until(
            &format!("session {session_id} stop"),
            Duration::from_secs(15),
            || {
                let harness = self.clone();
                let path = format!("/api/v1/sessions/{session_id}");
                async move {
                    match harness.delete_json_outcome(&path).await? {
                        JsonOutcome { status, body } if status == StatusCode::OK => Ok(Some(body)),
                        JsonOutcome { status, .. } if status == StatusCode::CONFLICT => {
                            let _ = harness.clear_bridge_control_session().await;
                            Ok(None)
                        }
                        JsonOutcome { status, body } => Err(anyhow!(
                            "DELETE {path} returned unexpected status {status} {body}"
                        )),
                    }
                }
            },
        )
        .await
    }

    pub fn unique_name(&self, prefix: &str) -> String {
        format!("{prefix}-{}", Uuid::now_v7())
    }

    pub fn repo_root(&self) -> &Path {
        self.repo_root.as_ref()
    }

    pub fn container_visible_path(&self, host_path: &Path) -> Result<String> {
        let relative = host_path.strip_prefix(self.repo_root()).with_context(|| {
            format!(
                "path {} is outside repo root {}",
                host_path.display(),
                self.repo_root().display()
            )
        })?;
        let relative = relative
            .iter()
            .map(|part| part.to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        Ok(format!(
            "{}/{}",
            self.container_workspace_root.trim_end_matches('/'),
            relative
        ))
    }

    pub async fn create_local_workflow_repo(&self) -> Result<LocalWorkflowRepo> {
        self.create_custom_workflow_repo(&[(
            "workflows/smoke/run.mjs",
            r#"export default async function run({ page, input, sessionId, workflowRunId, automationTaskId, artifacts }) {
  const targetUrl =
    input && typeof input.target_url === 'string' && input.target_url.trim()
      ? input.target_url.trim()
      : 'http://web:8080';
  const outputWorkspaceId =
    input && typeof input.output_workspace_id === 'string' && input.output_workspace_id.trim()
      ? input.output_workspace_id.trim()
      : null;
  if (!outputWorkspaceId) {
    throw new Error('workflow e2e requires input.output_workspace_id');
  }
  await page.goto(targetUrl, { waitUntil: 'networkidle' });
  const title = await page.title();
  const uploaded = await artifacts.uploadTextFile({
    workspaceId: outputWorkspaceId,
    fileName: 'workflow-compose-e2e-summary.txt',
    mediaType: 'text/plain; charset=utf-8',
    provenance: {
      origin: 'bpane-gateway-compose-e2e',
      kind: 'produced_file',
    },
    text: `title=${title}\nurl=${page.url()}\nsession=${sessionId}\nrun=${workflowRunId}\n`,
  });
  return {
    title,
    final_url: page.url(),
    session_id: sessionId,
    workflow_run_id: workflowRunId,
    automation_task_id: automationTaskId,
    output_file_name: uploaded.file_name,
    output_file_id: uploaded.file_id,
    output_workspace_id: uploaded.workspace_id,
  };
}
"#,
        )])
        .await
    }

    pub async fn create_custom_workflow_repo(
        &self,
        files: &[(&str, &str)],
    ) -> Result<LocalWorkflowRepo> {
        let temp_root = self.repo_root().join(".tmp");
        std::fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create temp root {}", temp_root.display()))?;
        let temp_dir = Builder::new()
            .prefix("bpane-gateway-compose-e2e-")
            .tempdir_in(&temp_root)
            .context("failed to create workflow temp dir")?;
        for (relative_path, contents) in files {
            let file_path = temp_dir.path().join(relative_path);
            let parent = file_path
                .parent()
                .ok_or_else(|| anyhow!("workflow fixture path {relative_path} has no parent"))?;
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
            std::fs::write(&file_path, contents).with_context(|| {
                format!("failed to write workflow fixture {}", file_path.display())
            })?;
        }

        initialize_git_repository(temp_dir.path())?;
        let commit = run_git_command(temp_dir.path(), &["rev-parse", "HEAD"])
            .context("failed to resolve workflow repo HEAD")?
            .trim()
            .to_string();
        let repository_url = self.container_visible_path(temp_dir.path())?;

        Ok(LocalWorkflowRepo {
            _temp_dir: temp_dir,
            repository_url,
            commit,
        })
    }

    pub fn create_compose_visible_file(
        &self,
        file_name: &str,
        bytes: &[u8],
    ) -> Result<ComposeVisibleFile> {
        let temp_root = self.repo_root().join(".tmp");
        std::fs::create_dir_all(&temp_root)
            .with_context(|| format!("failed to create temp root {}", temp_root.display()))?;
        let temp_dir = Builder::new()
            .prefix("bpane-gateway-compose-file-")
            .tempdir_in(&temp_root)
            .context("failed to create compose visible temp dir")?;
        let file_path = temp_dir.path().join(file_name);
        std::fs::write(&file_path, bytes)
            .with_context(|| format!("failed to write {}", file_path.display()))?;
        let container_path = self.container_visible_path(&file_path)?;
        Ok(ComposeVisibleFile {
            _temp_dir: temp_dir,
            container_path,
        })
    }

    async fn send_json<T: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<T>,
        headers: Option<HeaderMap>,
    ) -> Result<Value> {
        let response = self
            .send_request(method.clone(), path, body, headers)
            .await?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read response body")?;
        if !status.is_success() {
            bail!("{} {} returned {} {}", method, path, status, text);
        }
        serde_json::from_str(&text)
            .with_context(|| format!("failed to decode JSON response from {path}: {text}"))
    }

    async fn send_json_outcome<T: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<T>,
        headers: Option<HeaderMap>,
    ) -> Result<JsonOutcome> {
        let response = self
            .send_request(method.clone(), path, body, headers)
            .await?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read response body")?;
        let body = serde_json::from_str(&text).with_context(|| {
            format!(
                "failed to decode JSON response from {method} {path} with status {status}: {text}"
            )
        })?;
        Ok(JsonOutcome { status, body })
    }

    async fn send_bytes(
        &self,
        method: Method,
        path: &str,
        bytes: Vec<u8>,
        headers: HeaderMap,
    ) -> Result<Value> {
        let mut request = self
            .client
            .request(method.clone(), self.api_url(path))
            .bearer_auth(self.bearer_token())
            .body(bytes);
        request = request.headers(headers);
        let response = request
            .send()
            .await
            .with_context(|| format!("failed to call {method} {path}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read response body")?;
        if !status.is_success() {
            bail!("{} {} returned {} {}", method, path, status, text);
        }
        serde_json::from_str(&text)
            .with_context(|| format!("failed to decode JSON response from {path}: {text}"))
    }

    async fn send_for_bytes(
        &self,
        method: Method,
        path: &str,
        headers: Option<HeaderMap>,
    ) -> Result<Vec<u8>> {
        let response = self
            .send_request(method.clone(), path, None::<Value>, headers)
            .await?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .context("failed to read byte response")?;
        if !status.is_success() {
            let detail = String::from_utf8_lossy(&bytes);
            bail!("{} {} returned {} {}", method, path, status, detail);
        }
        Ok(bytes.to_vec())
    }

    async fn send_request<T: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<T>,
        headers: Option<HeaderMap>,
    ) -> Result<reqwest::Response> {
        let mut request = self
            .client
            .request(method.clone(), self.api_url(path))
            .bearer_auth(self.bearer_token());
        if let Some(headers) = headers {
            request = request.headers(headers);
        }
        if let Some(body) = body {
            request = request.json(&body);
        }
        request
            .send()
            .await
            .with_context(|| format!("failed to call {method} {path}"))
    }

    async fn send_bridge_json<T: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<T>,
    ) -> Result<Value> {
        let outcome = self
            .send_bridge_json_outcome(method.clone(), path, body)
            .await?;
        if !outcome.status.is_success() {
            bail!(
                "{} {} returned {} {}",
                method,
                path,
                outcome.status,
                outcome.body
            );
        }
        Ok(outcome.body)
    }

    async fn send_bridge_json_outcome<T: serde::Serialize>(
        &self,
        method: Method,
        path: &str,
        body: Option<T>,
    ) -> Result<JsonOutcome> {
        let mut request = self
            .client
            .request(method.clone(), self.mcp_bridge_url(path));
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .with_context(|| format!("failed to call MCP bridge {method} {path}"))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .context("failed to read MCP bridge response body")?;
        let body = serde_json::from_str(&text).with_context(|| {
            format!(
                "failed to decode MCP bridge JSON response from {method} {path} with status {status}: {text}"
            )
        })?;
        Ok(JsonOutcome { status, body })
    }
}

#[allow(dead_code)]
impl Drop for ComposeServiceRestoreGuard {
    fn drop(&mut self) {
        if let Err(error) = recreate_compose_services_blocking(&self.repo_root, &self.services, &[])
        {
            eprintln!(
                "[compose-api-surface] failed to restore compose services {:?}: {error}",
                self.services
            );
        }
    }
}

#[allow(dead_code)]
fn recreate_compose_services_blocking(
    repo_root: &Path,
    services: &[String],
    env_overrides: &[(&str, &str)],
) -> Result<()> {
    let mut command = Command::new("docker");
    command.args(["compose", "-f", "deploy/compose.yml", "up", "-d", "--build"]);
    for (key, value) in env_overrides {
        command.env(key, value);
    }
    for service in services {
        command.arg(service);
    }
    let status = command
        .current_dir(repo_root)
        .status()
        .context("failed to recreate compose services")?;
    if !status.success() {
        bail!("docker compose up failed while recreating services");
    }
    Ok(())
}

pub fn json_id(value: &Value, field: &str) -> Result<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow!("missing string field {field} in {value}"))
}

pub fn json_array<'a>(value: &'a Value, field: &str) -> Result<&'a Vec<Value>> {
    value
        .get(field)
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow!("missing array field {field} in {value}"))
}

pub async fn poll_until<F, Fut, T>(description: &str, timeout: Duration, mut fetch: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Option<T>>>,
{
    let started = std::time::Instant::now();
    loop {
        if let Some(value) = fetch().await? {
            return Ok(value);
        }
        if started.elapsed() >= timeout {
            bail!("timed out waiting for {description}");
        }
        sleep(Duration::from_millis(500)).await;
    }
}

fn repository_root() -> Result<PathBuf> {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    crate_root
        .parent()
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            anyhow!(
                "failed to resolve repository root from {}",
                crate_root.display()
            )
        })
}

async fn fetch_client_credentials_token(
    client: &reqwest::Client,
    token_url: &str,
    client_id: &str,
    client_secret: &str,
) -> Result<String> {
    let response = client
        .post(token_url)
        .form(&[
            ("grant_type", "client_credentials"),
            ("client_id", client_id),
            ("client_secret", client_secret),
        ])
        .send()
        .await
        .with_context(|| format!("failed to request token from {token_url}"))?;
    let status = response.status();
    let body: Value = response
        .json()
        .await
        .context("failed to decode token response")?;
    if status != StatusCode::OK {
        bail!("token request failed with {status}: {body}");
    }
    json_id(&body, "access_token")
}

fn initialize_git_repository(repo_dir: &Path) -> Result<()> {
    let init_status = Command::new("git")
        .args(["init", "-q", "-b", "main"])
        .current_dir(repo_dir)
        .status()
        .context("failed to initialize git repository with main branch")?;
    if !init_status.success() {
        let fallback_status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(repo_dir)
            .status()
            .context("failed to initialize git repository")?;
        if !fallback_status.success() {
            bail!("git init failed for {}", repo_dir.display());
        }
        let checkout_status = Command::new("git")
            .args(["checkout", "-q", "-b", "main"])
            .current_dir(repo_dir)
            .status()
            .context("failed to create main branch")?;
        if !checkout_status.success() {
            bail!("git checkout -b main failed for {}", repo_dir.display());
        }
    }

    run_git_command(
        repo_dir,
        &["config", "user.name", "BrowserPane Compose E2E"],
    )?;
    run_git_command(
        repo_dir,
        &["config", "user.email", "compose-e2e@browserpane.local"],
    )?;
    run_git_command(repo_dir, &["add", "."])?;
    run_git_command(
        repo_dir,
        &["commit", "-m", "Add compose e2e workflow fixture"],
    )?;
    Ok(())
}

fn run_git_command(repo_dir: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo_dir)
        .output()
        .with_context(|| format!("failed to run git {:?} in {}", args, repo_dir.display()))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "git {:?} failed in {}: {}",
            args,
            repo_dir.display(),
            stderr
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

pub fn map_headers(values: &[(&str, &str)]) -> Result<HeaderMap> {
    let mut headers = HeaderMap::new();
    for (name, value) in values {
        headers.insert(
            HeaderName::from_bytes(name.as_bytes())
                .with_context(|| format!("invalid header name {name}"))?,
            HeaderValue::from_str(value)
                .with_context(|| format!("invalid header value for {name}"))?,
        );
    }
    Ok(headers)
}

pub fn label_map(scope: &str) -> HashMap<String, String> {
    HashMap::from([
        ("suite".to_string(), "bpane-gateway-compose-e2e".to_string()),
        ("scope".to_string(), scope.to_string()),
    ])
}

pub fn recording_policy(mode: &str) -> Value {
    json!({
        "mode": mode,
        "format": "webm",
    })
}
