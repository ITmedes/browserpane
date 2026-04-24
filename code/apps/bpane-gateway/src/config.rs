use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug, Clone)]
#[command(
    name = "bpane-gateway",
    about = "BrowserPane WebTransport gateway server"
)]
pub struct Config {
    /// Path to the host agent Unix socket.
    #[arg(long, default_value = "/tmp/bpane.sock")]
    pub agent_socket: PathBuf,

    /// Runtime backend: "static_single", "docker_single", or "docker_pool".
    #[arg(long, default_value = "static_single")]
    pub runtime_backend: String,

    /// Idle timeout before an unattached runtime assignment is released or shut down.
    #[arg(long, default_value_t = 300)]
    pub runtime_idle_timeout_secs: u64,

    /// Optional CDP endpoint exposed for the static_single runtime backend.
    /// This should be reachable from internal automation services such as mcp-bridge.
    #[arg(long)]
    pub runtime_cdp_endpoint: Option<String>,

    /// TLS certificate file (PEM).
    #[arg(long)]
    pub cert: Option<PathBuf>,

    /// TLS private key file (PEM).
    #[arg(long)]
    pub key: Option<PathBuf>,

    /// Port to listen on for WebTransport connections.
    #[arg(long, default_value_t = 4433)]
    pub port: u16,

    /// Bind address.
    #[arg(long, default_value = "0.0.0.0")]
    pub bind: String,

    /// HMAC secret for token validation (hex-encoded).
    /// If not provided, a random secret is generated.
    #[arg(long)]
    pub hmac_secret: Option<String>,

    /// OIDC issuer URL used to validate JWT access tokens.
    /// When set, the gateway switches from legacy HMAC tokens to OIDC/JWT auth.
    #[arg(long)]
    pub oidc_issuer: Option<String>,

    /// Expected audience for OIDC JWT access tokens.
    #[arg(long)]
    pub oidc_audience: Option<String>,

    /// Optional JWKS URL override for OIDC providers.
    /// Useful when the public issuer is browser-reachable but the gateway must fetch keys over an internal URL.
    #[arg(long)]
    pub oidc_jwks_url: Option<String>,

    /// Optional Postgres connection string for the versioned session control plane.
    #[arg(long)]
    pub database_url: Option<String>,

    /// Public browser-facing gateway URL returned in session connect metadata.
    #[arg(long, default_value = "https://localhost:4433")]
    pub public_gateway_url: String,

    /// Docker CLI binary used by the optional docker_single runtime backend.
    #[arg(long, default_value = "docker")]
    pub docker_runtime_bin: String,

    /// Host worker image used by the optional docker_single runtime backend.
    #[arg(long)]
    pub docker_runtime_image: Option<String>,

    /// Docker network used by the optional docker_single runtime backend.
    #[arg(long)]
    pub docker_runtime_network: Option<String>,

    /// Docker named volume mounted at /run/bpane for the optional docker_single runtime backend.
    #[arg(long)]
    pub docker_runtime_volume: Option<String>,

    /// Container name prefix used by docker-backed runtime workers.
    #[arg(long, default_value = "bpane-runtime")]
    pub docker_runtime_container_name_prefix: String,

    /// Session-scoped socket root inside the shared run volume for docker_single.
    #[arg(long, default_value = "/run/bpane/sessions")]
    pub docker_runtime_socket_root: String,

    /// CDP proxy port exposed by docker-backed runtime workers.
    #[arg(long, default_value_t = 9223)]
    pub docker_runtime_cdp_proxy_port: u16,

    /// shm-size passed to docker run for the optional docker_single runtime backend.
    #[arg(long, default_value = "128m")]
    pub docker_runtime_shm_size: String,

    /// Startup timeout for the optional docker_single runtime backend.
    #[arg(long, default_value_t = 60)]
    pub docker_runtime_start_timeout_secs: u64,

    /// Maximum number of runtime-backed sessions that can exist in parallel in docker_pool mode.
    #[arg(long, default_value_t = 1)]
    pub max_active_runtimes: usize,

    /// Maximum number of runtime workers that may be starting concurrently in docker_pool mode.
    #[arg(long, default_value_t = 1)]
    pub max_starting_runtimes: usize,

    /// Optional env-file forwarded to docker run for the optional docker_single runtime backend.
    #[arg(long)]
    pub docker_runtime_env_file: Option<PathBuf>,

    /// Apply --security-opt seccomp=unconfined when launching docker runtime workers.
    #[arg(long, default_value_t = false)]
    pub docker_runtime_seccomp_unconfined: bool,

    /// Lifetime for minted session-scoped connect tickets.
    #[arg(long, default_value_t = 300)]
    pub session_ticket_ttl_secs: u64,

    /// Session heartbeat timeout in seconds.
    #[arg(long, default_value_t = 15)]
    pub heartbeat_timeout_secs: u64,

    /// Write the generated legacy dev token to this file.
    /// Ignored when OIDC auth is enabled.
    #[arg(long)]
    pub token_file: Option<PathBuf>,

    /// Port to listen on for the HTTP API (MCP bridge communication).
    #[arg(long, default_value_t = 8932)]
    pub api_port: u16,

    /// Maximum number of non-owner browser viewers allowed in a shared session.
    #[arg(long, default_value_t = 10)]
    pub max_viewers: u32,

    /// When enabled, the first browser client owns the session and later
    /// browser clients join as restricted viewers.
    #[arg(long, default_value_t = false)]
    pub exclusive_browser_owner: bool,

    /// Executable used to launch the optional recorder worker sidecar for recording.mode=always.
    #[arg(long)]
    pub recording_worker_bin: Option<PathBuf>,

    /// Additional argument passed to the recorder worker executable. Repeat for multiple args.
    #[arg(long = "recording-worker-arg")]
    pub recording_worker_args: Vec<String>,

    /// Chrome/Chromium executable path forwarded to the recorder worker.
    #[arg(long)]
    pub recording_worker_chrome: Option<PathBuf>,

    /// Local API base URL used by the recorder worker to talk back to the gateway.
    #[arg(long, default_value = "http://127.0.0.1:8932")]
    pub recording_worker_api_url: String,

    /// Browser page URL used by the recorder worker for the embedded client page.
    #[arg(long, default_value = "http://127.0.0.1:8080")]
    pub recording_worker_page_url: String,

    /// Output root where the recorder worker stores local artifacts before API finalization.
    #[arg(long, default_value = "/tmp/bpane-recordings")]
    pub recording_worker_output_root: PathBuf,

    /// Managed local root for finalized recording artifacts served by the gateway's local_fs artifact store.
    #[arg(long, default_value = "/tmp/bpane-recording-artifacts")]
    pub recording_artifact_local_root: PathBuf,

    /// Managed local root for persisted file workspace content served by the gateway's local_fs workspace file store.
    #[arg(long, default_value = "/tmp/bpane-file-workspaces")]
    pub file_workspace_local_root: PathBuf,

    /// Git executable used to resolve workflow git sources to immutable commits.
    #[arg(long, default_value = "git")]
    pub workflow_git_bin: PathBuf,

    /// Optional SPKI pin forwarded to the recorder worker Chromium process.
    #[arg(long)]
    pub recording_worker_cert_spki: Option<String>,

    /// Whether the recorder worker should run Chromium headless.
    #[arg(long, action = clap::ArgAction::Set, default_value_t = true)]
    pub recording_worker_headless: bool,

    /// Recorder worker connection timeout in seconds.
    #[arg(long, default_value_t = 30)]
    pub recording_worker_connect_timeout_secs: u64,

    /// Recorder worker polling interval for recording state updates in milliseconds.
    #[arg(long, default_value_t = 2000)]
    pub recording_worker_poll_interval_ms: u64,

    /// How long the gateway waits for recorder finalization during session teardown.
    #[arg(long, default_value_t = 30)]
    pub recording_worker_finalize_timeout_secs: u64,

    /// How often the gateway scans for completed recording artifacts whose per-session retention has expired.
    /// Set to 0 to disable artifact retention cleanup.
    #[arg(long, default_value_t = 60)]
    pub recording_artifact_cleanup_interval_secs: u64,

    /// Optional static bearer token forwarded to the recorder worker for gateway API access.
    #[arg(long)]
    pub recording_worker_bearer_token: Option<String>,

    /// Optional OIDC token URL forwarded to the recorder worker when gateway auth is OIDC.
    #[arg(long)]
    pub recording_worker_oidc_token_url: Option<String>,

    /// Optional OIDC client id forwarded to the recorder worker when gateway auth is OIDC.
    #[arg(long)]
    pub recording_worker_oidc_client_id: Option<String>,

    /// Optional OIDC client secret forwarded to the recorder worker when gateway auth is OIDC.
    #[arg(long)]
    pub recording_worker_oidc_client_secret: Option<String>,

    /// Optional OIDC scopes forwarded to the recorder worker when gateway auth is OIDC.
    #[arg(long)]
    pub recording_worker_oidc_scopes: Option<String>,
}
