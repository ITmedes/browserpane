use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;
use crate::session_access::token_codec::{
    PurposeBoundTokenCodec, PurposeBoundTokenError, TokenPurpose,
};

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
    codec: PurposeBoundTokenCodec,
    ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAutomationAccessTokenError {
    Malformed,
    WrongPurpose,
    InvalidSignature,
    Expired,
    InvalidPayload,
}

impl std::fmt::Display for SessionAutomationAccessTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => write!(f, "malformed session automation access token"),
            Self::WrongPurpose => write!(f, "wrong session automation access token purpose"),
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
        Self {
            codec: PurposeBoundTokenCodec::new(&secret, TokenPurpose::SessionAutomation),
            ttl,
        }
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
        Ok(IssuedSessionAutomationAccessToken {
            token: self.codec.encode(&claims).map_err(map_codec_error)?,
            expires_at,
            claims,
        })
    }

    pub fn validate_token(
        &self,
        token: &str,
    ) -> Result<SessionAutomationAccessTokenClaims, SessionAutomationAccessTokenError> {
        let claims: SessionAutomationAccessTokenClaims =
            self.codec.decode(token).map_err(map_codec_error)?;
        if Utc::now().timestamp() > claims.expires_at_unix {
            return Err(SessionAutomationAccessTokenError::Expired);
        }
        Ok(claims)
    }
}

fn map_codec_error(error: PurposeBoundTokenError) -> SessionAutomationAccessTokenError {
    match error {
        PurposeBoundTokenError::Malformed => SessionAutomationAccessTokenError::Malformed,
        PurposeBoundTokenError::WrongPurpose => SessionAutomationAccessTokenError::WrongPurpose,
        PurposeBoundTokenError::InvalidSignature => {
            SessionAutomationAccessTokenError::InvalidSignature
        }
        PurposeBoundTokenError::InvalidPayload => SessionAutomationAccessTokenError::InvalidPayload,
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
            safe_claims: Default::default(),
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
        let expired = manager.codec.encode(&claims).unwrap();
        assert_eq!(
            manager.validate_token(&expired),
            Err(SessionAutomationAccessTokenError::Expired)
        );
    }
}
