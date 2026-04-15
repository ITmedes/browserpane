use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::time::{SystemTime, UNIX_EPOCH};

type HmacSha256 = Hmac<Sha256>;

/// Token format: `{timestamp_hex}.{signature_hex}`
/// Timestamp is seconds since UNIX epoch.
/// Signature = HMAC-SHA256(secret, timestamp_bytes).
// Development convenience: keep tokens valid for a full day to avoid frequent reconnect issues.
const TOKEN_TTL_SECS: u64 = 2_592_000; // 30 days

#[derive(Debug, Clone)]
pub struct TokenValidator {
    secret: Vec<u8>,
}

impl TokenValidator {
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

        // Verify HMAC
        let mut mac = HmacSha256::new_from_slice(&self.secret).unwrap();
        mac.update(&timestamp_bytes);
        mac.verify_slice(&signature)
            .map_err(|_| AuthError::InvalidSignature)?;

        // Check TTL
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        // Reject tokens from the future (clock skew tolerance: 30s)
        if timestamp > now + 30 {
            return Err(AuthError::Expired);
        }
        if now.saturating_sub(timestamp) > TOKEN_TTL_SECS {
            return Err(AuthError::Expired);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    MalformedToken,
    InvalidSignature,
    Expired,
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedToken => write!(f, "malformed token"),
            Self::InvalidSignature => write!(f, "invalid token signature"),
            Self::Expired => write!(f, "token expired"),
        }
    }
}

impl std::error::Error for AuthError {}

#[cfg(test)]
mod tests;
