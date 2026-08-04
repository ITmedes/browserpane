use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;
use crate::session_access::token_codec::{
    PurposeBoundTokenCodec, PurposeBoundTokenError, TokenPurpose,
};

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
    codec: PurposeBoundTokenCodec,
    ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionConnectTicketError {
    Malformed,
    WrongPurpose,
    InvalidSignature,
    Expired,
    InvalidPayload,
}

impl std::fmt::Display for SessionConnectTicketError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => write!(f, "malformed session connect ticket"),
            Self::WrongPurpose => write!(f, "wrong session connect ticket purpose"),
            Self::InvalidSignature => write!(f, "invalid session connect ticket signature"),
            Self::Expired => write!(f, "session connect ticket expired"),
            Self::InvalidPayload => write!(f, "invalid session connect ticket payload"),
        }
    }
}

impl std::error::Error for SessionConnectTicketError {}

impl SessionConnectTicketManager {
    pub fn new(secret: Vec<u8>, ttl: Duration) -> Self {
        Self {
            codec: PurposeBoundTokenCodec::new(&secret, TokenPurpose::SessionConnect),
            ttl,
        }
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
        Ok(IssuedSessionConnectTicket {
            token: self.codec.encode(&claims).map_err(map_codec_error)?,
            expires_at,
            claims,
        })
    }

    pub fn validate_ticket(
        &self,
        token: &str,
    ) -> Result<SessionConnectTicketClaims, SessionConnectTicketError> {
        let claims: SessionConnectTicketClaims =
            self.codec.decode(token).map_err(map_codec_error)?;
        if Utc::now().timestamp() > claims.expires_at_unix {
            return Err(SessionConnectTicketError::Expired);
        }
        Ok(claims)
    }
}

fn map_codec_error(error: PurposeBoundTokenError) -> SessionConnectTicketError {
    match error {
        PurposeBoundTokenError::Malformed => SessionConnectTicketError::Malformed,
        PurposeBoundTokenError::WrongPurpose => SessionConnectTicketError::WrongPurpose,
        PurposeBoundTokenError::InvalidSignature => SessionConnectTicketError::InvalidSignature,
        PurposeBoundTokenError::InvalidPayload => SessionConnectTicketError::InvalidPayload,
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
            safe_claims: Default::default(),
        }
    }

    #[test]
    fn issues_and_validates_ticket() {
        let manager = SessionConnectTicketManager::new(vec![7; 32], Duration::from_secs(300));
        let session_id = Uuid::now_v7();
        let issued = manager
            .issue_ticket(session_id, &principal("demo"))
            .unwrap();

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
        let expired = manager.codec.encode(&claims).unwrap();
        assert_eq!(
            manager.validate_ticket(&expired),
            Err(SessionConnectTicketError::Expired)
        );
    }
}
