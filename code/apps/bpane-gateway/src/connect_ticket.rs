use std::time::Duration;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use chrono::{DateTime, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;

type TicketHmacSha256 = Hmac<Sha256>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionConnectTicketClaims {
    pub session_id: Uuid,
    pub subject: String,
    pub issuer: String,
    pub client_id: Option<String>,
    pub expires_at_unix: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedSessionConnectTicket {
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub claims: SessionConnectTicketClaims,
}

#[derive(Debug, Clone)]
pub struct SessionConnectTicketManager {
    secret: Vec<u8>,
    ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionConnectTicketError {
    Malformed,
    InvalidSignature,
    Expired,
    InvalidPayload,
}

impl std::fmt::Display for SessionConnectTicketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => write!(f, "malformed session connect ticket"),
            Self::InvalidSignature => write!(f, "invalid session connect ticket signature"),
            Self::Expired => write!(f, "session connect ticket expired"),
            Self::InvalidPayload => write!(f, "invalid session connect ticket payload"),
        }
    }
}

impl std::error::Error for SessionConnectTicketError {}

impl SessionConnectTicketManager {
    pub fn new(secret: Vec<u8>, ttl: Duration) -> Self {
        Self { secret, ttl }
    }

    pub fn issue_ticket(
        &self,
        session_id: Uuid,
        principal: &AuthenticatedPrincipal,
    ) -> Result<IssuedSessionConnectTicket, SessionConnectTicketError> {
        let expires_at = Utc::now()
            + chrono::Duration::from_std(self.ttl)
                .map_err(|_| SessionConnectTicketError::InvalidPayload)?;
        let claims = SessionConnectTicketClaims {
            session_id,
            subject: principal.subject.clone(),
            issuer: principal.issuer.clone(),
            client_id: principal.client_id.clone(),
            expires_at_unix: expires_at.timestamp(),
        };
        let payload = serde_json::to_vec(&claims).map_err(|_| SessionConnectTicketError::InvalidPayload)?;
        let payload_encoded = URL_SAFE_NO_PAD.encode(&payload);
        let signature_encoded = URL_SAFE_NO_PAD.encode(self.sign(&payload)?);
        Ok(IssuedSessionConnectTicket {
            token: format!("v1.{payload_encoded}.{signature_encoded}"),
            expires_at,
            claims,
        })
    }

    pub fn validate_ticket(
        &self,
        token: &str,
    ) -> Result<SessionConnectTicketClaims, SessionConnectTicketError> {
        let mut parts = token.split('.');
        let version = parts.next().ok_or(SessionConnectTicketError::Malformed)?;
        let payload_encoded = parts.next().ok_or(SessionConnectTicketError::Malformed)?;
        let signature_encoded = parts.next().ok_or(SessionConnectTicketError::Malformed)?;
        if parts.next().is_some() || version != "v1" {
            return Err(SessionConnectTicketError::Malformed);
        }

        let payload = URL_SAFE_NO_PAD
            .decode(payload_encoded)
            .map_err(|_| SessionConnectTicketError::Malformed)?;
        let signature = URL_SAFE_NO_PAD
            .decode(signature_encoded)
            .map_err(|_| SessionConnectTicketError::Malformed)?;
        let expected_signature = self.sign(&payload)?;
        if signature != expected_signature {
            return Err(SessionConnectTicketError::InvalidSignature);
        }

        let claims: SessionConnectTicketClaims =
            serde_json::from_slice(&payload).map_err(|_| SessionConnectTicketError::InvalidPayload)?;
        if Utc::now().timestamp() > claims.expires_at_unix {
            return Err(SessionConnectTicketError::Expired);
        }
        Ok(claims)
    }

    fn sign(&self, payload: &[u8]) -> Result<Vec<u8>, SessionConnectTicketError> {
        let mut mac = TicketHmacSha256::new_from_slice(&self.secret)
            .map_err(|_| SessionConnectTicketError::InvalidPayload)?;
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
            client_id: None,
        }
    }

    #[test]
    fn issues_and_validates_ticket() {
        let manager = SessionConnectTicketManager::new(vec![7; 32], Duration::from_secs(300));
        let session_id = Uuid::now_v7();
        let issued = manager.issue_ticket(session_id, &principal("demo")).unwrap();

        let validated = manager.validate_ticket(&issued.token).unwrap();
        assert_eq!(validated.session_id, session_id);
        assert_eq!(validated.subject, "demo");
    }

    #[test]
    fn rejects_tampered_ticket() {
        let manager = SessionConnectTicketManager::new(vec![7; 32], Duration::from_secs(300));
        let issued = manager
            .issue_ticket(Uuid::now_v7(), &principal("demo"))
            .unwrap();
        let mut tampered = issued.token;
        tampered.push('x');
        assert_eq!(
            manager.validate_ticket(&tampered),
            Err(SessionConnectTicketError::InvalidSignature)
        );
    }

    #[test]
    fn rejects_expired_ticket() {
        let manager = SessionConnectTicketManager::new(vec![7; 32], Duration::from_secs(0));
        let issued = manager
            .issue_ticket(Uuid::now_v7(), &principal("demo"))
            .unwrap();
        let mut claims = manager.validate_ticket(&issued.token).unwrap();
        claims.expires_at_unix = Utc::now().timestamp() - 1;
        let payload = serde_json::to_vec(&claims).unwrap();
        let payload_encoded = URL_SAFE_NO_PAD.encode(&payload);
        let signature_encoded = URL_SAFE_NO_PAD.encode(manager.sign(&payload).unwrap());
        let expired = format!("v1.{payload_encoded}.{signature_encoded}");
        assert_eq!(
            manager.validate_ticket(&expired),
            Err(SessionConnectTicketError::Expired)
        );
    }
}
