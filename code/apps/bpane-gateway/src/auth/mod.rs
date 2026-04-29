use std::sync::Arc;

mod hmac;
mod oidc;

pub use hmac::HmacTokenValidator;
pub use oidc::OidcTokenValidator;

#[cfg(test)]
pub(crate) use hmac::HmacSha256;

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
