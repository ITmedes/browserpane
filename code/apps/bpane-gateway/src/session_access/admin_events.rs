use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::auth::{AuthenticatedPrincipal, AuthenticatedPrincipalClaims};
use crate::session_access::token_codec::{
    PurposeBoundTokenCodec, PurposeBoundTokenError, TokenPurpose,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdminEventAccessTokenClaims {
    pub subject: String,
    pub issuer: String,
    pub display_name: Option<String>,
    pub client_id: Option<String>,
    pub safe_claims: AuthenticatedPrincipalClaims,
    pub expires_at_unix: i64,
}

impl AdminEventAccessTokenClaims {
    pub fn into_principal(self) -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: self.subject,
            issuer: self.issuer,
            display_name: self.display_name,
            client_id: self.client_id,
            safe_claims: self.safe_claims,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssuedAdminEventAccessToken {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct AdminEventAccessTokenManager {
    codec: PurposeBoundTokenCodec,
    ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminEventAccessTokenError {
    Malformed,
    WrongPurpose,
    InvalidSignature,
    Expired,
    InvalidPayload,
}

impl std::fmt::Display for AdminEventAccessTokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => write!(f, "malformed admin event access token"),
            Self::WrongPurpose => write!(f, "wrong admin event access token purpose"),
            Self::InvalidSignature => write!(f, "invalid admin event access token signature"),
            Self::Expired => write!(f, "admin event access token expired"),
            Self::InvalidPayload => write!(f, "invalid admin event access token payload"),
        }
    }
}

impl std::error::Error for AdminEventAccessTokenError {}

impl AdminEventAccessTokenManager {
    pub fn new(secret: Vec<u8>, ttl: Duration) -> Self {
        Self {
            codec: PurposeBoundTokenCodec::new(&secret, TokenPurpose::AdminEvents),
            ttl,
        }
    }

    pub fn issue_token(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<IssuedAdminEventAccessToken, AdminEventAccessTokenError> {
        let expires_at = Utc::now()
            + chrono::Duration::from_std(self.ttl)
                .map_err(|_| AdminEventAccessTokenError::InvalidPayload)?;
        let claims = AdminEventAccessTokenClaims {
            subject: principal.subject.clone(),
            issuer: principal.issuer.clone(),
            display_name: principal.display_name.clone(),
            client_id: principal.client_id.clone(),
            safe_claims: principal.safe_claims.clone(),
            expires_at_unix: expires_at.timestamp(),
        };
        Ok(IssuedAdminEventAccessToken {
            token: self.codec.encode(&claims).map_err(map_codec_error)?,
            expires_at,
        })
    }

    pub fn validate_token(
        &self,
        token: &str,
    ) -> Result<AdminEventAccessTokenClaims, AdminEventAccessTokenError> {
        let claims: AdminEventAccessTokenClaims =
            self.codec.decode(token).map_err(map_codec_error)?;
        if Utc::now().timestamp() > claims.expires_at_unix {
            return Err(AdminEventAccessTokenError::Expired);
        }
        Ok(claims)
    }
}

fn map_codec_error(error: PurposeBoundTokenError) -> AdminEventAccessTokenError {
    match error {
        PurposeBoundTokenError::Malformed => AdminEventAccessTokenError::Malformed,
        PurposeBoundTokenError::WrongPurpose => AdminEventAccessTokenError::WrongPurpose,
        PurposeBoundTokenError::InvalidSignature => AdminEventAccessTokenError::InvalidSignature,
        PurposeBoundTokenError::InvalidPayload => AdminEventAccessTokenError::InvalidPayload,
    }
}

#[cfg(test)]
mod tests {
    use crate::auth::{AuthenticatedPrincipalClaimValue, AuthenticatedPrincipalClaims};

    use super::*;

    fn principal() -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: "operator".to_string(),
            issuer: "https://issuer.example".to_string(),
            display_name: Some("Operator".to_string()),
            client_id: Some("bpane-admin".to_string()),
            safe_claims: AuthenticatedPrincipalClaims {
                groups: vec!["operators".to_string()],
                claims: vec![AuthenticatedPrincipalClaimValue {
                    name: "organization_id".to_string(),
                    value: "org-a".to_string(),
                }],
            },
        }
    }

    #[test]
    fn preserves_the_bounded_principal_contract() {
        let manager = AdminEventAccessTokenManager::new(vec![5; 32], Duration::from_secs(60));
        let issued = manager.issue_token(&principal()).unwrap();

        assert!(issued.token.starts_with("v2.admin-events."));
        assert_eq!(
            manager
                .validate_token(&issued.token)
                .unwrap()
                .into_principal(),
            principal()
        );
    }

    #[test]
    fn rejects_expired_tokens() {
        let manager = AdminEventAccessTokenManager::new(vec![5; 32], Duration::from_secs(0));
        let issued = manager.issue_token(&principal()).unwrap();
        let mut claims = manager.validate_token(&issued.token).unwrap();
        claims.expires_at_unix = Utc::now().timestamp() - 1;
        let expired = manager.codec.encode(&claims).unwrap();

        assert_eq!(
            manager.validate_token(&expired),
            Err(AdminEventAccessTokenError::Expired)
        );
    }
}
