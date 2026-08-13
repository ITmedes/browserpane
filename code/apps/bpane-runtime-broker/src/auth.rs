use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use thiserror::Error;
use tokio::sync::RwLock;
use tokio::time::sleep;

const JWT_CLOCK_SKEW_SECS: u64 = 30;
const MAX_BEARER_TOKEN_BYTES: usize = 16_384;
const STARTUP_ATTEMPTS: usize = 30;

/// OIDC settings for gateway-to-broker service authentication.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct OidcAuthenticatorConfig {
    /// Exact token issuer.
    pub issuer: String,
    /// Exact broker audience.
    pub audience: String,
    /// Optional internal JWKS endpoint override.
    pub jwks_url: Option<String>,
    /// Exact authorized gateway OAuth client id.
    pub allowed_client_id: String,
}

/// Authenticated service identity retained by request handling.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ServicePrincipal {
    /// OIDC subject.
    pub subject: String,
    /// Authorized OAuth client id.
    pub client_id: String,
    /// Token identifier used only for bounded replay correlation.
    pub token_id: String,
    /// Token expiry as Unix time.
    pub expires_at: u64,
}

/// Stable authentication failure codes.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum AuthenticationErrorCode {
    /// Authorization header or JWT shape is invalid.
    #[error("service credential is malformed")]
    MalformedToken,
    /// JWT signature is invalid.
    #[error("service credential signature is invalid")]
    InvalidSignature,
    /// JWT is expired or not active yet.
    #[error("service credential is expired")]
    Expired,
    /// JWT issuer is not approved.
    #[error("service credential issuer is invalid")]
    InvalidIssuer,
    /// JWT audience is not the runtime broker.
    #[error("service credential audience is invalid")]
    InvalidAudience,
    /// JWT signing algorithm is not approved.
    #[error("service credential algorithm is not approved")]
    UnsupportedAlgorithm,
    /// JWT does not identify a signing key.
    #[error("service credential is missing a key id")]
    MissingKeyId,
    /// JWT signing key is unknown after refresh.
    #[error("service credential signing key is unknown")]
    UnknownKeyId,
    /// OIDC signing keys cannot be fetched.
    #[error("service identity keys are unavailable")]
    KeyFetchFailed,
    /// OIDC signing keys cannot be parsed safely.
    #[error("service identity keys are invalid")]
    KeyParseFailed,
    /// Required subject, client, expiry, or token id is absent.
    #[error("service credential identity claims are incomplete")]
    MissingIdentityClaims,
    /// Authenticated OAuth client is not the configured gateway client.
    #[error("service client is not authorized")]
    ClientNotAllowed,
}

/// Sanitized authentication failure.
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
#[error("runtime broker authentication failed: {code}")]
pub struct AuthenticationError {
    /// Stable failure code.
    pub code: AuthenticationErrorCode,
}

impl From<AuthenticationErrorCode> for AuthenticationError {
    fn from(code: AuthenticationErrorCode) -> Self {
        Self { code }
    }
}

/// Authenticates internal broker callers without exposing token contents.
#[async_trait]
pub trait BrokerAuthenticator: Send + Sync {
    /// Validates a bearer credential and returns its constrained service identity.
    async fn authenticate(&self, token: &str) -> Result<ServicePrincipal, AuthenticationError>;
}

#[derive(Debug, Deserialize)]
struct OidcDiscoveryDocument {
    issuer: String,
    jwks_uri: String,
}

#[derive(Debug)]
struct OidcEndpoints {
    issuer: String,
    jwks_url: String,
}

/// OIDC/JWKS-backed broker authenticator using asymmetric JWT signatures.
pub struct OidcBrokerAuthenticator {
    issuer: String,
    audience: String,
    allowed_client_id: String,
    jwks_url: String,
    client: Client,
    keys: RwLock<HashMap<String, Arc<DecodingKey>>>,
}

impl OidcBrokerAuthenticator {
    /// Resolves OIDC metadata and loads initial asymmetric signing keys.
    ///
    /// # Errors
    ///
    /// Returns an error when discovery or JWKS loading remains unavailable
    /// after the bounded startup retry window.
    pub async fn new(config: OidcAuthenticatorConfig) -> anyhow::Result<Self> {
        let client = Client::builder().build()?;
        let endpoints = fetch_endpoints_with_retry(&client, &config).await?;
        let authenticator = Self {
            issuer: endpoints.issuer,
            audience: config.audience,
            allowed_client_id: config.allowed_client_id,
            jwks_url: endpoints.jwks_url,
            client,
            keys: RwLock::new(HashMap::new()),
        };
        authenticator
            .refresh_keys_with_retry()
            .await
            .map_err(anyhow::Error::new)?;
        Ok(authenticator)
    }

    async fn decode_claims(&self, token: &str) -> Result<Value, AuthenticationError> {
        if token.is_empty() || token.len() > MAX_BEARER_TOKEN_BYTES {
            return Err(AuthenticationErrorCode::MalformedToken.into());
        }
        let header = decode_header(token).map_err(|_| AuthenticationErrorCode::MalformedToken)?;
        let key_id = header.kid.ok_or(AuthenticationErrorCode::MissingKeyId)?;
        let algorithm = parse_asymmetric_algorithm(header.alg)?;
        let key = match self.lookup_key(&key_id).await {
            Some(key) => key,
            None => {
                self.refresh_keys().await?;
                self.lookup_key(&key_id)
                    .await
                    .ok_or(AuthenticationErrorCode::UnknownKeyId)?
            }
        };
        let mut validation = Validation::new(algorithm);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_audience(&[self.audience.as_str()]);
        validation.leeway = JWT_CLOCK_SKEW_SECS;
        validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);

        decode::<Value>(token, &key, &validation)
            .map(|data| data.claims)
            .map_err(map_jwt_error)
    }

    async fn lookup_key(&self, key_id: &str) -> Option<Arc<DecodingKey>> {
        self.keys.read().await.get(key_id).cloned()
    }

    async fn refresh_keys(&self) -> Result<(), AuthenticationError> {
        let key_set = self
            .client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|_| AuthenticationErrorCode::KeyFetchFailed)?
            .error_for_status()
            .map_err(|_| AuthenticationErrorCode::KeyFetchFailed)?
            .json::<JwkSet>()
            .await
            .map_err(|_| AuthenticationErrorCode::KeyFetchFailed)?;
        let mut next = HashMap::new();
        for key in key_set.keys {
            let Some(key_id) = key.common.key_id.clone() else {
                continue;
            };
            if !matches!(
                key.algorithm,
                AlgorithmParameters::RSA(_) | AlgorithmParameters::EllipticCurve(_)
            ) {
                continue;
            }
            let decoding_key =
                DecodingKey::from_jwk(&key).map_err(|_| AuthenticationErrorCode::KeyParseFailed)?;
            next.insert(key_id, Arc::new(decoding_key));
        }
        if next.is_empty() {
            return Err(AuthenticationErrorCode::KeyParseFailed.into());
        }
        *self.keys.write().await = next;
        Ok(())
    }

    async fn refresh_keys_with_retry(&self) -> Result<(), AuthenticationError> {
        let mut last_error = AuthenticationErrorCode::KeyFetchFailed.into();
        for attempt in 0..STARTUP_ATTEMPTS {
            match self.refresh_keys().await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    last_error = error;
                    if attempt + 1 < STARTUP_ATTEMPTS {
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }
        Err(last_error)
    }
}

#[async_trait]
impl BrokerAuthenticator for OidcBrokerAuthenticator {
    async fn authenticate(&self, token: &str) -> Result<ServicePrincipal, AuthenticationError> {
        let claims = self.decode_claims(token).await?;
        let principal = principal_from_claims(&claims)?;
        if principal.client_id != self.allowed_client_id {
            return Err(AuthenticationErrorCode::ClientNotAllowed.into());
        }
        Ok(principal)
    }
}

fn principal_from_claims(claims: &Value) -> Result<ServicePrincipal, AuthenticationError> {
    let claim = |name: &str| claims.get(name).and_then(Value::as_str);
    let subject = claim("sub").ok_or(AuthenticationErrorCode::MissingIdentityClaims)?;
    let client_id = claim("azp")
        .or_else(|| claim("client_id"))
        .ok_or(AuthenticationErrorCode::MissingIdentityClaims)?;
    let token_id = claim("jti").ok_or(AuthenticationErrorCode::MissingIdentityClaims)?;
    let expires_at = claims
        .get("exp")
        .and_then(Value::as_u64)
        .ok_or(AuthenticationErrorCode::MissingIdentityClaims)?;
    if subject.is_empty() || client_id.is_empty() || token_id.is_empty() {
        return Err(AuthenticationErrorCode::MissingIdentityClaims.into());
    }
    Ok(ServicePrincipal {
        subject: subject.to_string(),
        client_id: client_id.to_string(),
        token_id: token_id.to_string(),
        expires_at,
    })
}

fn parse_asymmetric_algorithm(algorithm: Algorithm) -> Result<Algorithm, AuthenticationError> {
    match algorithm {
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::ES256
        | Algorithm::ES384 => Ok(algorithm),
        _ => Err(AuthenticationErrorCode::UnsupportedAlgorithm.into()),
    }
}

fn map_jwt_error(error: jsonwebtoken::errors::Error) -> AuthenticationError {
    use jsonwebtoken::errors::ErrorKind;

    let code = match error.kind() {
        ErrorKind::ExpiredSignature | ErrorKind::ImmatureSignature => {
            AuthenticationErrorCode::Expired
        }
        ErrorKind::InvalidIssuer => AuthenticationErrorCode::InvalidIssuer,
        ErrorKind::InvalidAudience => AuthenticationErrorCode::InvalidAudience,
        ErrorKind::InvalidSignature => AuthenticationErrorCode::InvalidSignature,
        _ => AuthenticationErrorCode::MalformedToken,
    };
    code.into()
}

async fn fetch_endpoints(
    client: &Client,
    config: &OidcAuthenticatorConfig,
) -> anyhow::Result<OidcEndpoints> {
    if let Some(jwks_url) = &config.jwks_url {
        return Ok(OidcEndpoints {
            issuer: config.issuer.clone(),
            jwks_url: jwks_url.clone(),
        });
    }
    let discovery_url = format!(
        "{}/.well-known/openid-configuration",
        config.issuer.trim_end_matches('/')
    );
    let document = client
        .get(discovery_url)
        .send()
        .await?
        .error_for_status()?
        .json::<OidcDiscoveryDocument>()
        .await?;
    if document.issuer != config.issuer {
        anyhow::bail!("OIDC discovery issuer does not match configured issuer");
    }
    Ok(OidcEndpoints {
        issuer: document.issuer,
        jwks_url: document.jwks_uri,
    })
}

async fn fetch_endpoints_with_retry(
    client: &Client,
    config: &OidcAuthenticatorConfig,
) -> anyhow::Result<OidcEndpoints> {
    let mut last_error = None;
    for attempt in 0..STARTUP_ATTEMPTS {
        match fetch_endpoints(client, config).await {
            Ok(endpoints) => return Ok(endpoints),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < STARTUP_ATTEMPTS {
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("OIDC discovery failed")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extracts_required_service_identity_claims() {
        let claims = json!({
            "sub": "service-account-gateway",
            "azp": "bpane-runtime-broker-gateway",
            "jti": "token-1",
            "exp": 4_000_000_000_u64
        });
        assert_eq!(
            principal_from_claims(&claims).unwrap(),
            ServicePrincipal {
                subject: "service-account-gateway".to_string(),
                client_id: "bpane-runtime-broker-gateway".to_string(),
                token_id: "token-1".to_string(),
                expires_at: 4_000_000_000,
            }
        );
    }

    #[test]
    fn rejects_incomplete_service_identity_claims() {
        for missing in ["sub", "azp", "jti", "exp"] {
            let mut claims = json!({
                "sub": "service-account-gateway",
                "azp": "bpane-runtime-broker-gateway",
                "jti": "token-1",
                "exp": 4_000_000_000_u64
            });
            claims.as_object_mut().unwrap().remove(missing);
            assert_eq!(
                principal_from_claims(&claims),
                Err(AuthenticationErrorCode::MissingIdentityClaims.into())
            );
        }
    }

    #[test]
    fn rejects_symmetric_or_unapproved_algorithms() {
        assert_eq!(
            parse_asymmetric_algorithm(Algorithm::HS256),
            Err(AuthenticationErrorCode::UnsupportedAlgorithm.into())
        );
        assert!(parse_asymmetric_algorithm(Algorithm::RS256).is_ok());
        assert!(parse_asymmetric_algorithm(Algorithm::ES256).is_ok());
    }
}
