use std::path::PathBuf;

use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct WorkflowConfig {
    /// Git executable used to resolve workflow git sources to immutable commits.
    #[arg(long = "workflow-git-bin", default_value = "git")]
    pub workflow_git_bin: PathBuf,

    /// Docker CLI binary used to launch short-lived workflow worker jobs.
    #[arg(long = "workflow-worker-docker-bin", default_value = "docker")]
    pub workflow_worker_docker_bin: PathBuf,

    /// Docker image used for automatic workflow worker execution.
    #[arg(long = "workflow-worker-image")]
    pub workflow_worker_image: Option<String>,

    /// Maximum number of automatic workflow workers that may run concurrently.
    /// Set to 0 to disable workflow-worker admission limits.
    #[arg(long = "workflow-worker-max-active", default_value_t = 0)]
    pub workflow_worker_max_active: usize,

    /// Docker network used by automatic workflow worker jobs.
    #[arg(long = "workflow-worker-network")]
    pub workflow_worker_network: Option<String>,

    /// Container name prefix used by automatic workflow worker jobs.
    #[arg(
        long = "workflow-worker-container-name-prefix",
        default_value = "bpane-workflow"
    )]
    pub workflow_worker_container_name_prefix: String,

    /// API base URL used by automatic workflow workers to talk back to the gateway.
    #[arg(
        long = "workflow-worker-api-url",
        default_value = "http://gateway:8932"
    )]
    pub workflow_worker_api_url: String,

    /// Work root inside the workflow worker container for downloaded source snapshots and runner state.
    #[arg(
        long = "workflow-worker-work-root",
        default_value = "/tmp/bpane-workflows"
    )]
    pub workflow_worker_work_root: PathBuf,

    /// Optional static bearer token forwarded to workflow workers for gateway API access.
    #[arg(long = "workflow-worker-bearer-token")]
    pub workflow_worker_bearer_token: Option<String>,

    /// Optional OIDC token URL forwarded to workflow workers when gateway auth is OIDC.
    #[arg(long = "workflow-worker-oidc-token-url")]
    pub workflow_worker_oidc_token_url: Option<String>,

    /// Optional OIDC client id forwarded to workflow workers when gateway auth is OIDC.
    #[arg(long = "workflow-worker-oidc-client-id")]
    pub workflow_worker_oidc_client_id: Option<String>,

    /// Optional OIDC client secret forwarded to workflow workers when gateway auth is OIDC.
    #[arg(long = "workflow-worker-oidc-client-secret")]
    pub workflow_worker_oidc_client_secret: Option<String>,

    /// Optional OIDC scopes forwarded to workflow workers when gateway auth is OIDC.
    #[arg(long = "workflow-worker-oidc-scopes")]
    pub workflow_worker_oidc_scopes: Option<String>,

    /// Poll interval in milliseconds for outbound workflow event delivery.
    #[arg(
        long = "workflow-event-delivery-poll-interval-ms",
        default_value_t = 1000
    )]
    pub workflow_event_delivery_poll_interval_ms: u64,

    /// HTTP timeout in seconds for outbound workflow event delivery.
    #[arg(long = "workflow-event-delivery-timeout-secs", default_value_t = 10)]
    pub workflow_event_delivery_timeout_secs: u64,

    /// Maximum number of attempts for each outbound workflow event delivery.
    #[arg(long = "workflow-event-delivery-max-attempts", default_value_t = 6)]
    pub workflow_event_delivery_max_attempts: u32,

    /// Maximum number of due workflow event deliveries claimed per dispatch pass.
    #[arg(long = "workflow-event-delivery-batch-size", default_value_t = 32)]
    pub workflow_event_delivery_batch_size: usize,

    /// Base backoff in seconds for retrying outbound workflow event delivery.
    #[arg(
        long = "workflow-event-delivery-base-backoff-secs",
        default_value_t = 2
    )]
    pub workflow_event_delivery_base_backoff_secs: u64,

    /// How often the gateway scans for completed workflow runs whose retained logs or outputs have expired.
    /// Set to 0 to disable workflow retention cleanup.
    #[arg(
        long = "workflow-retention-cleanup-interval-secs",
        default_value_t = 300
    )]
    pub workflow_retention_cleanup_interval_secs: u64,

    /// How long completed workflow run logs remain queryable before cleanup removes them.
    /// Set to 0 to disable workflow log cleanup.
    #[arg(long = "workflow-log-retention-secs", default_value_t = 604800)]
    pub workflow_log_retention_secs: u64,

    /// How long completed workflow outputs remain queryable before cleanup clears them.
    /// Set to 0 to disable workflow output cleanup.
    #[arg(long = "workflow-output-retention-secs", default_value_t = 2592000)]
    pub workflow_output_retention_secs: u64,
}
