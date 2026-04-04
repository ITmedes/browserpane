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

    /// Session heartbeat timeout in seconds.
    #[arg(long, default_value_t = 15)]
    pub heartbeat_timeout_secs: u64,

    /// Write the generated dev token to this file (for use by frontends).
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
}
