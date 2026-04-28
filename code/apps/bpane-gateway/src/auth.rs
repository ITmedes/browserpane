use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use sha2::Sha256;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

type HmacSha256 = Hmac<Sha256>;

// Development convenience: keep tokens valid for a full day to avoid frequent reconnect issues.
const TOKEN_TTL_SECS: u64 = 2_592_000; // 30 days
const JWT_CLOCK_SKEW_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct OidcConfig {
    pub issuer: String,
    pub audience: String,
    pub jwks_url: Option<String>,
}

#[derive(Clone)]
pub struct AuthValidator {
    mode: AuthMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthenticatedPrincipal {
    pub subject: String,
    pub issuer: String,
    pub display_name: Option<String>,
    pub client_id: Option<String>,
}

#[derive(Clone)]
enum AuthMode {
    Hmac(HmacTokenValidator),
    Oidc(Arc<OidcTokenValidator>),
}

impl AuthValidator {
    pub fn from_hmac_secret(secret: Vec<u8>) -> Self {
        Self {
            mode: AuthMode::Hmac(HmacTokenValidator::new(secret)),
        }
    }

    pub async fn from_oidc(config: OidcConfig) -> anyhow::Result<Self> {
        Ok(Self {
            mode: AuthMode::Oidc(Arc::new(OidcTokenValidator::new(config).await?)),
        })
    }

    pub fn generate_token(&self) -> Option<String> {
        match &self.mode {
            AuthMode::Hmac(validator) => Some(validator.generate_token()),
            AuthMode::Oidc(_) => None,
        }
    }

    pub fn is_oidc(&self) -> bool {
        matches!(self.mode, AuthMode::Oidc(_))
    }

    pub async fn authenticate(&self, token: &str) -> Result<AuthenticatedPrincipal, AuthError> {
        match &self.mode {
            AuthMode::Hmac(validator) => validator.authenticate(token),
            AuthMode::Oidc(validator) => validator.authenticate(token).await,
        }
    }
}

#[derive(Debug, Clone)]
pub struct HmacTokenValidator {
    secret: Vec<u8>,
}

impl HmacTokenValidator {
    pub fn new(secret: Vec<u8>) -> Self {
        Self { secret }
    }

    pub fn generate_token(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let timestamp_hex = hex::encode(now.to_le_bytes());
        let mut mac = HmacSha256::new_from_slice(&self.secret).unwrap();
        mac.update(&now.to_le_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        format!("{timestamp_hex}.{signature}")
    }

    pub fn validate_token(&self, token: &str) -> Result<(), AuthError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 2 {
            return Err(AuthError::MalformedToken);
        }

        let timestamp_bytes = hex::decode(parts[0]).map_err(|_| AuthError::MalformedToken)?;
        if timestamp_bytes.len() != 8 {
            return Err(AuthError::MalformedToken);
        }
        let mut ts_arr = [0u8; 8];
        ts_arr.copy_from_slice(&timestamp_bytes);
        let timestamp = u64::from_le_bytes(ts_arr);

        let signature = hex::decode(parts[1]).map_err(|_| AuthError::MalformedToken)?;

        let mut mac = HmacSha256::new_from_slice(&self.secret).unwrap();
        mac.update(&timestamp_bytes);
        mac.verify_slice(&signature)
            .map_err(|_| AuthError::InvalidSignature)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if timestamp > now + JWT_CLOCK_SKEW_SECS {
            return Err(AuthError::Expired);
        }
        if now.saturating_sub(timestamp) > TOKEN_TTL_SECS {
            return Err(AuthError::Expired);
        }

        Ok(())
    }

    pub fn authenticate(&self, token: &str) -> Result<AuthenticatedPrincipal, AuthError> {
        self.validate_token(token)?;
        let subject_suffix = token.split('.').next().unwrap_or(token);
        Ok(AuthenticatedPrincipal {
            subject: format!("legacy-dev-token:{subject_suffix}"),
            issuer: "bpane-gateway".to_string(),
            display_name: None,
            client_id: None,
        })
    }
}

struct OidcTokenValidator {
    issuer: String,
    audience: String,
    jwks_url: String,
    client: Client,
    keys: RwLock<HashMap<String, Arc<DecodingKey>>>,
}

#[derive(Debug, Deserialize)]
struct OidcDiscoveryDocument {
    issuer: String,
    jwks_uri: String,
}

impl OidcTokenValidator {
    async fn new(config: OidcConfig) -> anyhow::Result<Self> {
        let client = Client::builder().build()?;
        let (issuer, jwks_url) = fetch_oidc_endpoints_with_retry(&client, &config).await?;
        let validator = Self {
            issuer,
            audience: config.audience,
            jwks_url,
            client,
            keys: RwLock::new(HashMap::new()),
        };
        validator.refresh_keys_with_retry().await?;
        Ok(validator)
    }

    async fn authenticate(&self, token: &str) -> Result<AuthenticatedPrincipal, AuthError> {
        let claims = self.decode_claims(token).await?;
        let subject = claims
            .get("sub")
            .and_then(Value::as_str)
            .or_else(|| claims.get("preferred_username").and_then(Value::as_str))
            .or_else(|| claims.get("email").and_then(Value::as_str))
            .ok_or(AuthError::MalformedToken)?;
        let issuer = claims
            .get("iss")
            .and_then(Value::as_str)
            .ok_or(AuthError::MalformedToken)?;
        let display_name = claims
            .get("preferred_username")
            .and_then(Value::as_str)
            .or_else(|| claims.get("email").and_then(Value::as_str))
            .or_else(|| claims.get("client_id").and_then(Value::as_str))
            .or_else(|| claims.get("azp").and_then(Value::as_str))
            .map(ToString::to_string);
        let client_id = claims
            .get("client_id")
            .and_then(Value::as_str)
            .or_else(|| claims.get("azp").and_then(Value::as_str))
            .map(ToString::to_string);

        Ok(AuthenticatedPrincipal {
            subject: subject.to_string(),
            issuer: issuer.to_string(),
            display_name,
            client_id,
        })
    }

    async fn decode_claims(&self, token: &str) -> Result<Value, AuthError> {
        let header = decode_header(token).map_err(|_| AuthError::MalformedToken)?;
        let kid = header.kid.ok_or(AuthError::MissingKeyId)?;
        let algorithm = parse_asymmetric_algorithm(header.alg)?;

        let key = match self.lookup_key(&kid).await {
            Some(key) => key,
            None => {
                self.refresh_keys().await?;
                self.lookup_key(&kid).await.ok_or(AuthError::UnknownKeyId)?
            }
        };

        let mut validation = Validation::new(algorithm);
        validation.set_issuer(&[self.issuer.as_str()]);
        validation.set_audience(&[self.audience.as_str()]);
        validation.leeway = JWT_CLOCK_SKEW_SECS;

        decode::<Value>(token, &key, &validation)
            .map(|data| data.claims)
            .map_err(map_jwt_error)
    }

    async fn lookup_key(&self, kid: &str) -> Option<Arc<DecodingKey>> {
        self.keys.read().await.get(kid).cloned()
    }

    async fn refresh_keys(&self) -> Result<(), AuthError> {
        let key_set = self
            .client
            .get(&self.jwks_url)
            .send()
            .await
            .map_err(|_| AuthError::KeyFetchFailed)?
            .error_for_status()
            .map_err(|_| AuthError::KeyFetchFailed)?
            .json::<JwkSet>()
            .await
            .map_err(|_| AuthError::KeyFetchFailed)?;

        let mut next = HashMap::new();
        for jwk in key_set.keys {
            let Some(kid) = jwk.common.key_id.clone() else {
                continue;
            };
            let decoding_key =
                DecodingKey::from_jwk(&jwk).map_err(|_| AuthError::KeyParseFailed)?;
            if matches!(
                jwk.algorithm,
                AlgorithmParameters::RSA(_) | AlgorithmParameters::EllipticCurve(_)
            ) {
                next.insert(kid, Arc::new(decoding_key));
            }
        }

        if next.is_empty() {
            return Err(AuthError::KeyParseFailed);
        }

        *self.keys.write().await = next;
        Ok(())
    }

    async fn refresh_keys_with_retry(&self) -> Result<(), AuthError> {
        let max_attempts = 30;
        let mut last_error = AuthError::KeyFetchFailed;
        for attempt in 0..max_attempts {
            match self.refresh_keys().await {
                Ok(()) => return Ok(()),
                Err(error) => {
                    last_error = error;
                    if attempt + 1 < max_attempts {
                        sleep(Duration::from_secs(2)).await;
                    }
                }
            }
        }
        Err(last_error)
    }
}

async fn fetch_oidc_endpoints(
    client: &Client,
    config: &OidcConfig,
) -> anyhow::Result<(String, String)> {
    if let Some(jwks_url) = &config.jwks_url {
        return Ok((config.issuer.clone(), jwks_url.clone()));
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
    Ok((document.issuer, document.jwks_uri))
}

async fn fetch_oidc_endpoints_with_retry(
    client: &Client,
    config: &OidcConfig,
) -> anyhow::Result<(String, String)> {
    let max_attempts = 30;
    let mut last_error: Option<anyhow::Error> = None;
    for attempt in 0..max_attempts {
        match fetch_oidc_endpoints(client, config).await {
            Ok(endpoints) => return Ok(endpoints),
            Err(error) => {
                last_error = Some(error);
                if attempt + 1 < max_attempts {
                    sleep(Duration::from_secs(2)).await;
                }
            }
        }
    }
    Err(last_error.unwrap_or_else(|| anyhow::anyhow!("OIDC discovery failed")))
}

fn parse_asymmetric_algorithm(algorithm: Algorithm) -> Result<Algorithm, AuthError> {
    match algorithm {
        Algorithm::RS256
        | Algorithm::RS384
        | Algorithm::RS512
        | Algorithm::ES256
        | Algorithm::ES384 => Ok(algorithm),
        _ => Err(AuthError::UnsupportedAlgorithm),
    }
}

fn map_jwt_error(error: jsonwebtoken::errors::Error) -> AuthError {
    use jsonwebtoken::errors::ErrorKind;

    match error.kind() {
        ErrorKind::ExpiredSignature | ErrorKind::ImmatureSignature => AuthError::Expired,
        ErrorKind::InvalidIssuer => AuthError::InvalidIssuer,
        ErrorKind::InvalidAudience => AuthError::InvalidAudience,
        ErrorKind::InvalidSignature => AuthError::InvalidSignature,
        ErrorKind::InvalidToken | ErrorKind::InvalidAlgorithm | ErrorKind::Base64(_) => {
            AuthError::MalformedToken
        }
        _ => AuthError::MalformedToken,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    MalformedToken,
    InvalidSignature,
    Expired,
    InvalidIssuer,
    InvalidAudience,
    UnsupportedAlgorithm,
    MissingKeyId,
    UnknownKeyId,
    KeyFetchFailed,
    KeyParseFailed,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedToken => write!(f, "malformed token"),
            Self::InvalidSignature => write!(f, "invalid token signature"),
            Self::Expired => write!(f, "token expired"),
            Self::InvalidIssuer => write!(f, "invalid token issuer"),
            Self::InvalidAudience => write!(f, "invalid token audience"),
            Self::UnsupportedAlgorithm => write!(f, "unsupported token algorithm"),
            Self::MissingKeyId => write!(f, "token missing key id"),
            Self::UnknownKeyId => write!(f, "unknown token key id"),
            Self::KeyFetchFailed => write!(f, "failed to fetch token signing keys"),
            Self::KeyParseFailed => write!(f, "failed to parse token signing keys"),
        }
    }
}

impl std::error::Error for AuthError {}

#[cfg(test)]
mod tests;
