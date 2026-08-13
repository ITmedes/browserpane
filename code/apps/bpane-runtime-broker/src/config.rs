use std::net::SocketAddr;
use std::num::{NonZeroU64, NonZeroUsize};
use std::time::Duration;

use anyhow::{bail, Context};
use clap::Parser;

use crate::{BrokerApiSettings, LedgerConfig, OidcAuthenticatorConfig};

const MAX_REQUEST_LIMIT_BYTES: usize = 1_048_576;
const MAX_CONCURRENCY_LIMIT: usize = 1_024;
const MAX_TIMEOUT_SECS: u64 = 300;

/// Runtime broker command-line configuration.
#[derive(Debug, Clone, Parser)]
#[command(name = "bpane-runtime-broker")]
pub struct BrokerConfig {
    /// Internal HTTP listen address.
    #[arg(
        long,
        env = "BPANE_RUNTIME_BROKER_LISTEN",
        default_value = "0.0.0.0:8940"
    )]
    pub listen: SocketAddr,
    /// Expected OIDC issuer.
    #[arg(long, env = "BPANE_RUNTIME_BROKER_OIDC_ISSUER")]
    pub oidc_issuer: String,
    /// Expected broker token audience.
    #[arg(
        long,
        env = "BPANE_RUNTIME_BROKER_OIDC_AUDIENCE",
        default_value = "bpane-runtime-broker"
    )]
    pub oidc_audience: String,
    /// Optional internal JWKS URL when discovery is not externally reachable.
    #[arg(long, env = "BPANE_RUNTIME_BROKER_OIDC_JWKS_URL")]
    pub oidc_jwks_url: Option<String>,
    /// Only this OIDC client may invoke broker operations.
    #[arg(
        long,
        env = "BPANE_RUNTIME_BROKER_ALLOWED_CLIENT_ID",
        default_value = "bpane-runtime-broker-gateway"
    )]
    pub allowed_client_id: String,
    /// Maximum operation request body size.
    #[arg(
        long,
        env = "BPANE_RUNTIME_BROKER_MAX_REQUEST_BYTES",
        default_value_t = 65_536
    )]
    pub max_request_bytes: usize,
    /// Maximum concurrent operation executions.
    #[arg(
        long,
        env = "BPANE_RUNTIME_BROKER_MAX_CONCURRENT",
        default_value_t = 16
    )]
    pub max_concurrent: usize,
    /// Per-operation execution deadline.
    #[arg(
        long,
        env = "BPANE_RUNTIME_BROKER_OPERATION_TIMEOUT_SECS",
        default_value_t = 30
    )]
    pub operation_timeout_secs: u64,
    /// Maximum retained pending and completed idempotency entries.
    #[arg(
        long,
        env = "BPANE_RUNTIME_BROKER_LEDGER_CAPACITY",
        default_value_t = 4_096
    )]
    pub ledger_capacity: usize,
    /// Completed idempotency-result retention window.
    #[arg(
        long,
        env = "BPANE_RUNTIME_BROKER_LEDGER_TTL_SECS",
        default_value_t = 600
    )]
    pub ledger_ttl_secs: u64,
}

impl BrokerConfig {
    /// Validates configuration and builds API, ledger, and OIDC settings.
    ///
    /// # Errors
    ///
    /// Returns an error for empty identity values, zero limits, or limits above
    /// the broker's hard safety bounds.
    pub fn validated(
        &self,
    ) -> anyhow::Result<(BrokerApiSettings, LedgerConfig, OidcAuthenticatorConfig)> {
        for (name, value) in [
            ("OIDC issuer", self.oidc_issuer.as_str()),
            ("OIDC audience", self.oidc_audience.as_str()),
            ("allowed client id", self.allowed_client_id.as_str()),
        ] {
            if value.trim().is_empty() {
                bail!("{name} must not be empty");
            }
        }
        let max_request_bytes = nonzero_usize(self.max_request_bytes, "max request bytes")?;
        if max_request_bytes.get() > MAX_REQUEST_LIMIT_BYTES {
            bail!("max request bytes exceeds the 1 MiB safety bound");
        }
        let max_concurrent = nonzero_usize(self.max_concurrent, "max concurrent operations")?;
        if max_concurrent.get() > MAX_CONCURRENCY_LIMIT {
            bail!("max concurrent operations exceeds the safety bound");
        }
        let timeout_secs = nonzero_u64(self.operation_timeout_secs, "operation timeout")?;
        if timeout_secs.get() > MAX_TIMEOUT_SECS {
            bail!("operation timeout exceeds the 300 second safety bound");
        }
        let ledger_capacity = nonzero_usize(self.ledger_capacity, "ledger capacity")?;
        let ledger_ttl_secs = nonzero_u64(self.ledger_ttl_secs, "ledger TTL")?;

        Ok((
            BrokerApiSettings {
                max_request_bytes,
                max_concurrent,
                operation_timeout: Duration::from_secs(timeout_secs.get()),
            },
            LedgerConfig {
                capacity: ledger_capacity,
                completed_ttl: Duration::from_secs(ledger_ttl_secs.get()),
            },
            OidcAuthenticatorConfig {
                issuer: self.oidc_issuer.clone(),
                audience: self.oidc_audience.clone(),
                jwks_url: self.oidc_jwks_url.clone(),
                allowed_client_id: self.allowed_client_id.clone(),
            },
        ))
    }
}

fn nonzero_usize(value: usize, name: &str) -> anyhow::Result<NonZeroUsize> {
    NonZeroUsize::new(value).with_context(|| format!("{name} must be greater than zero"))
}

fn nonzero_u64(value: u64, name: &str) -> anyhow::Result<NonZeroU64> {
    NonZeroU64::new(value).with_context(|| format!("{name} must be greater than zero"))
}

#[cfg(test)]
mod tests {
    use super::*;

    type ConfigMutation = Box<dyn Fn(&mut BrokerConfig)>;

    fn config() -> BrokerConfig {
        BrokerConfig {
            listen: "127.0.0.1:8940".parse().unwrap(),
            oidc_issuer: "https://issuer.example".to_string(),
            oidc_audience: "bpane-runtime-broker".to_string(),
            oidc_jwks_url: Some("https://issuer.example/jwks".to_string()),
            allowed_client_id: "bpane-runtime-broker-gateway".to_string(),
            max_request_bytes: 65_536,
            max_concurrent: 16,
            operation_timeout_secs: 30,
            ledger_capacity: 4_096,
            ledger_ttl_secs: 600,
        }
    }

    #[test]
    fn accepts_bounded_configuration() {
        config().validated().unwrap();
    }

    #[test]
    fn rejects_empty_identity_and_unsafe_limits() {
        let mutations: Vec<ConfigMutation> = vec![
            Box::new(|config| config.oidc_issuer.clear()),
            Box::new(|config| config.max_request_bytes = 0),
            Box::new(|config| config.max_request_bytes = MAX_REQUEST_LIMIT_BYTES + 1),
            Box::new(|config| config.max_concurrent = 0),
            Box::new(|config| config.max_concurrent = MAX_CONCURRENCY_LIMIT + 1),
            Box::new(|config| config.operation_timeout_secs = MAX_TIMEOUT_SECS + 1),
            Box::new(|config| config.ledger_capacity = 0),
            Box::new(|config| config.ledger_ttl_secs = 0),
        ];
        for mutate in mutations {
            let mut candidate = config();
            mutate(&mut candidate);
            assert!(candidate.validated().is_err());
        }
    }
}
