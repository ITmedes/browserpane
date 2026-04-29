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
}
