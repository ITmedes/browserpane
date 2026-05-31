use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};

use crate::auth::{
    AuthError, AuthenticatedPrincipal, AuthenticatedPrincipalClaimValue,
    AuthenticatedPrincipalClaims, OidcConfig,
};

const JWT_CLOCK_SKEW_SECS: u64 = 30;
const MAX_SAFE_CLAIM_VALUES: usize = 128;
const MAX_SAFE_CLAIM_VALUE_LEN: usize = 256;
const MAX_SAFE_CLAIM_NAME_LEN: usize = 128;
const SAFE_TOP_LEVEL_CLAIMS: &[&str] = &[
    "groups",
    "roles",
    "tenant",
    "tenant_id",
    "organization",
    "organization_id",
    "org_id",
    "department",
];

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
        let safe_claims = safe_principal_claims_from_token_claims(&claims);

        Ok(AuthenticatedPrincipal {
            subject: subject.to_string(),
            issuer: issuer.to_string(),
            display_name,
            client_id,
            safe_claims,
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

fn safe_principal_claims_from_token_claims(claims: &Value) -> AuthenticatedPrincipalClaims {
    let mut groups = BTreeSet::new();
    let mut claim_values = BTreeSet::new();

    for claim_name in SAFE_TOP_LEVEL_CLAIMS {
        for value in safe_string_values_at(claims, &[*claim_name]) {
            if *claim_name == "groups" {
                groups.insert(value.clone());
            }
            claim_values.insert(((*claim_name).to_string(), value));
        }
    }

    for value in safe_string_values_at(claims, &["realm_access", "roles"]) {
        claim_values.insert(("realm_access.roles".to_string(), value));
    }

    if let Some(resource_access) = claims.get("resource_access").and_then(Value::as_object) {
        for (client, client_access) in resource_access {
            let Some(client_segment) = sanitize_safe_claim_path_segment(client) else {
                continue;
            };
            let claim_name = format!("resource_access.{client_segment}.roles");
            for value in safe_string_values_at(client_access, &["roles"]) {
                claim_values.insert((claim_name.clone(), value));
            }
        }
    }

    AuthenticatedPrincipalClaims {
        groups: groups.into_iter().take(MAX_SAFE_CLAIM_VALUES).collect(),
        claims: claim_values
            .into_iter()
            .take(MAX_SAFE_CLAIM_VALUES)
            .map(|(name, value)| AuthenticatedPrincipalClaimValue { name, value })
            .collect(),
    }
}

fn safe_string_values_at(root: &Value, path: &[&str]) -> Vec<String> {
    let Some(value) = value_at_path(root, path) else {
        return Vec::new();
    };
    safe_string_values(value)
}

fn value_at_path<'a>(root: &'a Value, path: &[&str]) -> Option<&'a Value> {
    let mut current = root;
    for segment in path {
        current = current.get(*segment)?;
    }
    Some(current)
}

fn safe_string_values(value: &Value) -> Vec<String> {
    match value {
        Value::String(value) => sanitize_safe_claim_value(value).into_iter().collect(),
        Value::Array(values) => values
            .iter()
            .filter_map(Value::as_str)
            .filter_map(sanitize_safe_claim_value)
            .take(MAX_SAFE_CLAIM_VALUES)
            .collect(),
        _ => Vec::new(),
    }
}

fn sanitize_safe_claim_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_SAFE_CLAIM_VALUE_LEN
        || trimmed.chars().any(char::is_control)
    {
        return None;
    }
    Some(trimmed.to_string())
}

fn sanitize_safe_claim_path_segment(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_SAFE_CLAIM_NAME_LEN
        || trimmed
            .chars()
            .any(|character| !(character.is_ascii_alphanumeric() || "-_:.@".contains(character)))
    {
        return None;
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn safe_principal_claims_keep_only_allowlisted_string_values() {
        let claims = json!({
            "groups": ["customer-acme-support", " ", 7, "customer-acme-support"],
            "roles": "workspace-admin",
            "tenant": "acme",
            "realm_access": { "roles": ["offline_access", "uma_authorization"] },
            "resource_access": {
                "bpane-admin": { "roles": ["admin-ui"] },
                "bad/client": { "roles": ["ignored"] }
            },
            "address": { "street": "raw object is ignored" },
            "unsafe": ["not allowlisted"]
        });

        let safe_claims = safe_principal_claims_from_token_claims(&claims);

        assert_eq!(safe_claims.groups, vec!["customer-acme-support"]);
        assert!(safe_claims.has_claim_value("groups", "customer-acme-support"));
        assert!(safe_claims.has_claim_value("roles", "workspace-admin"));
        assert!(safe_claims.has_claim_value("tenant", "acme"));
        assert!(safe_claims.has_claim_value("realm_access.roles", "offline_access"));
        assert!(safe_claims.has_claim_value("resource_access.bpane-admin.roles", "admin-ui"));
        assert!(!safe_claims.has_claim_value("unsafe", "not allowlisted"));
        assert!(!safe_claims.has_claim_value("resource_access.bad/client.roles", "ignored"));
    }

    #[test]
    fn safe_principal_claims_drop_control_characters_and_long_values() {
        let claims = json!({
            "groups": [
                "customer-acme-support\u{0000}",
                "x".repeat(MAX_SAFE_CLAIM_VALUE_LEN + 1),
                "customer-beta-support"
            ]
        });

        let safe_claims = safe_principal_claims_from_token_claims(&claims);

        assert_eq!(safe_claims.groups, vec!["customer-beta-support"]);
        assert!(safe_claims.has_group("customer-beta-support"));
    }
}
