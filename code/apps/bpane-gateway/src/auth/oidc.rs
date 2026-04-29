use std::collections::HashMap;
use std::sync::Arc;

use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::auth::{AuthError, AuthenticatedPrincipal, OidcConfig};

const JWT_CLOCK_SKEW_SECS: u64 = 30;

#[derive(Debug, Deserialize)]
struct OidcDiscoveryResponse {
    issuer: String,
    jwks_uri: String,
}

#[derive(Debug)]
struct OidcEndpoints {
    issuer: String,
    jwks_url: String,
}

pub struct OidcTokenValidator {
    issuer: String,
    audience: String,
    jwks_url: String,
    client: Client,
    keys: RwLock<HashMap<String, Arc<DecodingKey>>>,
}

impl OidcTokenValidator {
    pub async fn new(config: OidcConfig) -> anyhow::Result<Self> {
        let client = Client::builder().build()?;
        let endpoints = fetch_oidc_endpoints_with_retry(&client, &config).await?;
        let validator = Self {
            issuer: endpoints.issuer,
            audience: config.audience,
            jwks_url: endpoints.jwks_url,
            client,
            keys: RwLock::new(HashMap::new()),
        };
        validator.refresh_keys_with_retry().await?;
        Ok(validator)
    }

    pub async fn authenticate(&self, token: &str) -> Result<AuthenticatedPrincipal, AuthError> {
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
        .json::<OidcDiscoveryResponse>()
        .await?;
    Ok(OidcEndpoints {
        issuer: document.issuer,
        jwks_url: document.jwks_uri,
    })
}

async fn fetch_oidc_endpoints_with_retry(
    client: &Client,
    config: &OidcConfig,
) -> anyhow::Result<OidcEndpoints> {
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
