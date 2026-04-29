use std::time::Duration;

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;

type AccessTokenHmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionAutomationAccessTokenClaims {
    pub session_id: Uuid,
    pub subject: String,
    pub issuer: String,
    pub client_id: Option<String>,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedSessionAutomationAccessToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub claims: SessionAutomationAccessTokenClaims,
}

#[derive(Debug, Clone)]
pub struct SessionAutomationAccessTokenManager {
    secret: Vec<u8>,
    ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAutomationAccessTokenError {
    Malformed,
    InvalidSignature,
    Expired,
    InvalidPayload,
}

impl std::fmt::Display for SessionAutomationAccessTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => write!(f, "malformed session automation access token"),
            Self::InvalidSignature => {
                write!(f, "invalid session automation access token signature")
            }
            Self::Expired => write!(f, "session automation access token expired"),
            Self::InvalidPayload => write!(f, "invalid session automation access token payload"),
        }
    }
}

impl std::error::Error for SessionAutomationAccessTokenError {}

impl SessionAutomationAccessTokenManager {
    pub fn new(secret: Vec<u8>, ttl: Duration) -> Self {
        Self { secret, ttl }
    }

    pub fn issue_token(
        &self,
        session_id: Uuid,
        principal: &AuthenticatedPrincipal,
    ) -> Result<IssuedSessionAutomationAccessToken, SessionAutomationAccessTokenError> {
        let expires_at = Utc::now()
            + chrono::Duration::from_std(self.ttl)
                .map_err(|_| SessionAutomationAccessTokenError::InvalidPayload)?;
        let claims = SessionAutomationAccessTokenClaims {
            session_id,
            subject: principal.subject.clone(),
            issuer: principal.issuer.clone(),
            client_id: principal.client_id.clone(),
            expires_at_unix: expires_at.timestamp(),
        };
        let payload = serde_json::to_vec(&claims)
            .map_err(|_| SessionAutomationAccessTokenError::InvalidPayload)?;
        let payload_encoded = URL_SAFE_NO_PAD.encode(&payload);
        let signature_encoded = URL_SAFE_NO_PAD.encode(self.sign(&payload)?);
        Ok(IssuedSessionAutomationAccessToken {
            token: format!("v1.{payload_encoded}.{signature_encoded}"),
            expires_at,
            claims,
        })
    }

    pub fn validate_token(
        &self,
        token: &str,
    ) -> Result<SessionAutomationAccessTokenClaims, SessionAutomationAccessTokenError> {
        let mut parts = token.split('.');
        let version = parts
            .next()
            .ok_or(SessionAutomationAccessTokenError::Malformed)?;
        let payload_encoded = parts
            .next()
            .ok_or(SessionAutomationAccessTokenError::Malformed)?;
        let signature_encoded = parts
            .next()
            .ok_or(SessionAutomationAccessTokenError::Malformed)?;
        if parts.next().is_some() || version != "v1" {
            return Err(SessionAutomationAccessTokenError::Malformed);
        }

        let payload = URL_SAFE_NO_PAD
            .decode(payload_encoded)
            .map_err(|_| SessionAutomationAccessTokenError::Malformed)?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature_encoded)
            .map_err(|_| SessionAutomationAccessTokenError::Malformed)?;
        let expected_signature = self.sign(&payload)?;
        if signature != expected_signature {
            return Err(SessionAutomationAccessTokenError::InvalidSignature);
        }

        let claims: SessionAutomationAccessTokenClaims = serde_json::from_slice(&payload)
            .map_err(|_| SessionAutomationAccessTokenError::InvalidPayload)?;
        if Utc::now().timestamp() > claims.expires_at_unix {
            return Err(SessionAutomationAccessTokenError::Expired);
        }
        Ok(claims)
    }

    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, SessionAutomationAccessTokenError> {
        let mut mac = AccessTokenHmacSha256::new_from_slice(&self.secret)
            .map_err(|_| SessionAutomationAccessTokenError::InvalidPayload)?;
        mac.update(payload);
        Ok(mac.finalize().into_bytes().to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn principal(subject: &str) -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: subject.to_string(),
            issuer: "issuer".to_string(),
            display_name: Some(subject.to_string()),
            client_id: Some("bpane-mcp-bridge".to_string()),
        }
    }

    #[test]
    fn issues_and_validates_token() {
        let manager =
            SessionAutomationAccessTokenManager::new(vec![9; 32], Duration::from_secs(300));
        let session_id = Uuid::now_v7();
        let issued = manager.issue_token(session_id, &principal("demo")).unwrap();

        let validated = manager.validate_token(&issued.token).unwrap();
        assert_eq!(validated.session_id, session_id);
        assert_eq!(validated.subject, "demo");
        assert_eq!(validated.client_id.as_deref(), Some("bpane-mcp-bridge"));
    }

    #[test]
    fn rejects_tampered_token() {
        let manager =
            SessionAutomationAccessTokenManager::new(vec![9; 32], Duration::from_secs(300));
        let issued = manager
            .issue_token(Uuid::now_v7(), &principal("demo"))
            .unwrap();
        let mut tampered = issued.token;
        tampered.push('x');
        assert_eq!(
            manager.validate_token(&tampered),
            Err(SessionAutomationAccessTokenError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_expired_token() {
        let manager = SessionAutomationAccessTokenManager::new(vec![9; 32], Duration::from_secs(0));
        let issued = manager
            .issue_token(Uuid::now_v7(), &principal("demo"))
            .unwrap();
        let mut claims = manager.validate_token(&issued.token).unwrap();
        claims.expires_at_unix = Utc::now().timestamp() - 1;
        let payload = serde_json::to_vec(&claims).unwrap();
        let payload_encoded = URL_SAFE_NO_PAD.encode(&payload);
        let signature_encoded = URL_SAFE_NO_PAD.encode(manager.sign(&payload).unwrap());
        let expired = format!("v1.{payload_encoded}.{signature_encoded}");
        assert_eq!(
            manager.validate_token(&expired),
            Err(SessionAutomationAccessTokenError::Expired)
        );
    }
}
