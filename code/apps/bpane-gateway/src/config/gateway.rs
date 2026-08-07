use std::path::PathBuf;

use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct GatewayConfig {
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

    /// Port to listen on for the HTTP API (MCP bridge communication).
    #[arg(long, default_value_t = 8932)]
    pub api_port: u16,

    /// Public browser-facing gateway URL returned in session connect metadata.
    #[arg(long, default_value = "https://localhost:4433")]
    pub public_gateway_url: String,

    /// Session heartbeat timeout in seconds.
    #[arg(long, default_value_t = 15)]
    pub heartbeat_timeout_secs: u64,

    /// Maximum number of non-owner browser viewers allowed in a shared session.
    #[arg(long, default_value_t = 10)]
    pub max_viewers: u32,

    /// When enabled, the first browser client owns the session and later
    /// browser clients join as restricted viewers.
    #[arg(long, default_value_t = false)]
    pub exclusive_browser_owner: bool,

    /// Internal MCP bridge control-session endpoint used by the gateway proxy.
    #[arg(long = "mcp-bridge-control-url")]
    pub mcp_bridge_control_url: Option<String>,

    /// Bearer token used by the gateway when calling the MCP bridge control endpoint.
    #[arg(long = "mcp-bridge-control-token")]
    pub mcp_bridge_control_token: Option<String>,

    /// Timeout for gateway-to-MCP-bridge control calls.
    #[arg(long = "mcp-bridge-control-timeout-secs", default_value_t = 5)]
    pub mcp_bridge_control_timeout_secs: u64,

    /// Maximum duration of one dependency readiness check.
    #[arg(long = "readiness-check-timeout-secs", default_value_t = 3)]
    pub readiness_check_timeout_secs: u64,

    /// Maximum duration allowed for active work to drain during shutdown.
    #[arg(long = "shutdown-drain-timeout-secs", default_value_t = 15)]
    pub shutdown_drain_timeout_secs: u64,

    /// Time to advertise not-ready before closing the HTTP listener.
    #[arg(long = "shutdown-readiness-grace-secs", default_value_t = 2)]
    pub shutdown_readiness_grace_secs: u64,

    /// Maximum compressed size of one browser-context import request.
    #[arg(
        long = "browser-context-import-max-archive-bytes",
        default_value_t = 536_870_912
    )]
    pub browser_context_import_max_archive_bytes: u64,

    /// Maximum compressed size of profile.tar.gz inside an import archive.
    #[arg(
        long = "browser-context-import-max-profile-archive-bytes",
        default_value_t = 536_870_912
    )]
    pub browser_context_import_max_profile_archive_bytes: u64,

    /// Maximum uncompressed tar stream size accepted from profile.tar.gz.
    #[arg(
        long = "browser-context-import-max-profile-uncompressed-bytes",
        default_value_t = 2_147_483_648
    )]
    pub browser_context_import_max_profile_uncompressed_bytes: u64,

    /// Maximum number of tar entries accepted from profile.tar.gz.
    #[arg(
        long = "browser-context-import-max-profile-entries",
        default_value_t = 100_000
    )]
    pub browser_context_import_max_profile_entries: usize,

    /// Maximum number of browser-context imports admitted concurrently.
    #[arg(long = "browser-context-import-max-concurrent", default_value_t = 2)]
    pub browser_context_import_max_concurrent: usize,
}
