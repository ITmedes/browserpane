use std::path::PathBuf;

use clap::Args;

#[derive(Args, Debug, Clone)]
pub struct AuthConfig {
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

    /// Lifetime for minted session-scoped connect tickets.
    #[arg(long, default_value_t = 300)]
    pub session_ticket_ttl_secs: u64,

    /// Write the generated legacy dev token to this file.
    /// Ignored when OIDC auth is enabled.
    #[arg(long)]
    pub token_file: Option<PathBuf>,
}
