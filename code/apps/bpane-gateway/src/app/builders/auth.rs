use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail};
use tracing::info;

use crate::auth::{AuthValidator, OidcConfig};
use crate::config::Config;
use crate::session_access::{SessionAutomationAccessTokenManager, SessionConnectTicketManager};

use super::AuthServices;

impl AuthServices {
    pub(in crate::app) async fn build(config: &Config) -> anyhow::Result<Self> {
        let shared_secret = load_or_generate_shared_secret(config)?;
        let auth_validator = Arc::new(build_auth_validator(config, &shared_secret).await?);
        Ok(Self {
            connect_ticket_manager: Arc::new(SessionConnectTicketManager::new(
                shared_secret.clone(),
                Duration::from_secs(config.session_ticket_ttl_secs),
            )),
            automation_access_token_manager: Arc::new(SessionAutomationAccessTokenManager::new(
                shared_secret,
                Duration::from_secs(config.session_ticket_ttl_secs),
            )),
            auth_validator,
        })
    }
}

pub(in crate::app) fn load_or_generate_shared_secret(config: &Config) -> anyhow::Result<Vec<u8>> {
    match &config.hmac_secret {
        Some(hex_secret) => {
            let decoded = hex::decode(hex_secret)?;
            if decoded.len() < 16 {
                bail!(
                    "HMAC secret must be at least 16 bytes (32 hex chars), got {}",
                    decoded.len()
                );
            }
            Ok(decoded)
        }
        None => {
            let mut secret = vec![0u8; 32];
            rand::fill(&mut secret[..]);
            Ok(secret)
        }
    }
}

async fn build_auth_validator(
    config: &Config,
    shared_secret: &[u8],
) -> anyhow::Result<AuthValidator> {
    if let Some(issuer) = &config.oidc_issuer {
        let audience = config
            .oidc_audience
            .clone()
            .ok_or_else(|| anyhow!("--oidc-audience is required when --oidc-issuer is set"))?;
        info!("using OIDC/JWT auth with issuer {}", issuer);
        if config.token_file.is_some() {
            info!("ignoring --token-file because OIDC auth is enabled");
        }
        AuthValidator::from_oidc(OidcConfig {
            issuer: issuer.clone(),
            audience,
            jwks_url: config.oidc_jwks_url.clone(),
        })
        .await
    } else {
        let validator = AuthValidator::from_hmac_secret(shared_secret.to_vec());
        if let Some(token) = validator.generate_token() {
            info!("generated dev token: {token}");
            if let Some(path) = &config.token_file {
                std::fs::write(path, &token)?;
                info!("wrote token to {}", path.display());
            }
        }
        Ok(validator)
    }
}
