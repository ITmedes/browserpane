use std::path::PathBuf;

use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct RecordingConfig {
    /// Executable used to launch the optional recorder worker sidecar for recording.mode=always.
    #[arg(long = "recording-worker-bin")]
    pub recording_worker_bin: Option<PathBuf>,

    /// Additional argument passed to the recorder worker executable. Repeat for multiple args.
    #[arg(long = "recording-worker-arg")]
    pub recording_worker_args: Vec<String>,

    /// Chrome/Chromium executable path forwarded to the recorder worker.
    #[arg(long = "recording-worker-chrome")]
    pub recording_worker_chrome: Option<PathBuf>,

    /// Local API base URL used by the recorder worker to talk back to the gateway.
    #[arg(
        long = "recording-worker-api-url",
        default_value = "http://127.0.0.1:8932"
    )]
    pub recording_worker_api_url: String,

    /// Browser page URL used by the recorder worker for the embedded client page.
    #[arg(
        long = "recording-worker-page-url",
        default_value = "http://127.0.0.1:8080"
    )]
    pub recording_worker_page_url: String,

    /// Output root where the recorder worker stores local artifacts before API finalization.
    #[arg(
        long = "recording-worker-output-root",
        default_value = "/tmp/bpane-recordings"
    )]
    pub recording_worker_output_root: PathBuf,

    /// Optional SPKI pin forwarded to the recorder worker Chromium process.
    #[arg(long = "recording-worker-cert-spki")]
    pub recording_worker_cert_spki: Option<String>,

    /// Whether the recorder worker should run Chromium headless.
    #[arg(long = "recording-worker-headless", action = clap::ArgAction::Set, default_value_t = true)]
    pub recording_worker_headless: bool,

    /// Recorder worker connection timeout in seconds.
    #[arg(long = "recording-worker-connect-timeout-secs", default_value_t = 30)]
    pub recording_worker_connect_timeout_secs: u64,

    /// Recorder worker polling interval for recording state updates in milliseconds.
    #[arg(long = "recording-worker-poll-interval-ms", default_value_t = 2000)]
    pub recording_worker_poll_interval_ms: u64,

    /// How long the gateway waits for recorder finalization during session teardown.
    #[arg(long = "recording-worker-finalize-timeout-secs", default_value_t = 30)]
    pub recording_worker_finalize_timeout_secs: u64,

    /// How often the gateway scans for completed recording artifacts whose per-session retention has expired.
    /// Set to 0 to disable artifact retention cleanup.
    #[arg(
        long = "recording-artifact-cleanup-interval-secs",
        default_value_t = 60
    )]
    pub recording_artifact_cleanup_interval_secs: u64,

    /// Optional static bearer token forwarded to the recorder worker for gateway API access.
    #[arg(long = "recording-worker-bearer-token")]
    pub recording_worker_bearer_token: Option<String>,

    /// Optional OIDC token URL forwarded to the recorder worker when gateway auth is OIDC.
    #[arg(long = "recording-worker-oidc-token-url")]
    pub recording_worker_oidc_token_url: Option<String>,

    /// Optional OIDC client id forwarded to the recorder worker when gateway auth is OIDC.
    #[arg(long = "recording-worker-oidc-client-id")]
    pub recording_worker_oidc_client_id: Option<String>,

    /// Optional OIDC client secret forwarded to the recorder worker when gateway auth is OIDC.
    #[arg(long = "recording-worker-oidc-client-secret")]
    pub recording_worker_oidc_client_secret: Option<String>,

    /// Optional OIDC scopes forwarded to the recorder worker when gateway auth is OIDC.
    #[arg(long = "recording-worker-oidc-scopes")]
    pub recording_worker_oidc_scopes: Option<String>,
}
