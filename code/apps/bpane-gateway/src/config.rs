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
}
