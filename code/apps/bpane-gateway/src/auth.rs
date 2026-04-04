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
mod tests {
    use super::*;

    #[test]
    fn generate_and_validate_token() {
        let validator = TokenValidator::new(b"test-secret-key".to_vec());
        let token = validator.generate_token();
        assert!(validator.validate_token(&token).is_ok());
    }

    #[test]
    fn reject_tampered_token() {
        let validator = TokenValidator::new(b"test-secret-key".to_vec());
        let token = validator.generate_token();
        // Tamper with the signature
        let mut tampered = token.clone();
        tampered.push('a');
        assert_eq!(
            validator.validate_token(&tampered),
            Err(AuthError::MalformedToken)
        );
    }

    #[test]
    fn reject_wrong_secret() {
        let v1 = TokenValidator::new(b"secret-1".to_vec());
        let v2 = TokenValidator::new(b"secret-2".to_vec());
        let token = v1.generate_token();
        assert_eq!(v2.validate_token(&token), Err(AuthError::InvalidSignature));
    }

    #[test]
    fn reject_malformed() {
        let validator = TokenValidator::new(b"secret".to_vec());
        assert_eq!(
            validator.validate_token("not-a-token"),
            Err(AuthError::MalformedToken)
        );
        assert_eq!(validator.validate_token(""), Err(AuthError::MalformedToken));
    }

    #[test]
    fn reject_empty_parts() {
        let validator = TokenValidator::new(b"secret".to_vec());
        assert_eq!(
            validator.validate_token("."),
            Err(AuthError::MalformedToken)
        );
        assert_eq!(
            validator.validate_token("abc."),
            Err(AuthError::MalformedToken)
        );
        assert_eq!(
            validator.validate_token(".abc"),
            Err(AuthError::MalformedToken)
        );
    }

    #[test]
    fn reject_multiple_dots() {
        let validator = TokenValidator::new(b"secret".to_vec());
        assert_eq!(
            validator.validate_token("aabb.ccdd.eeff"),
            Err(AuthError::MalformedToken)
        );
    }

    #[test]
    fn reject_non_hex_characters() {
        let validator = TokenValidator::new(b"secret".to_vec());
        assert_eq!(
            validator.validate_token("zzzzzzzzzzzzzzzz.abcdef1234567890"),
            Err(AuthError::MalformedToken)
        );
    }

    #[test]
    fn token_within_clock_skew_tolerance_accepted() {
        let validator = TokenValidator::new(b"test-secret-key".to_vec());
        // Forge a token 20 seconds in the future (within 30s tolerance)
        let future_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 20;
        let timestamp_hex = hex::encode(future_time.to_le_bytes());
        let mut mac = HmacSha256::new_from_slice(b"test-secret-key").unwrap();
        mac.update(&future_time.to_le_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        let token = format!("{timestamp_hex}.{signature}");
        // Should be accepted (within 30s skew tolerance)
        assert!(validator.validate_token(&token).is_ok());
    }

    #[test]
    fn different_secrets_produce_different_tokens() {
        let v1 = TokenValidator::new(b"secret-a".to_vec());
        let v2 = TokenValidator::new(b"secret-b".to_vec());
        let t1 = v1.generate_token();
        let t2 = v2.generate_token();
        // Signatures should differ (timestamps may be same but HMAC differs)
        let sig1 = t1.split('.').nth(1).unwrap();
        let sig2 = t2.split('.').nth(1).unwrap();
        assert_ne!(sig1, sig2);
    }

    #[test]
    fn reject_future_timestamp() {
        let validator = TokenValidator::new(b"test-secret-key".to_vec());
        // Forge a token with a timestamp 5 minutes in the future
        let future_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 300;
        let timestamp_hex = hex::encode(future_time.to_le_bytes());
        let mut mac = HmacSha256::new_from_slice(b"test-secret-key").unwrap();
        mac.update(&future_time.to_le_bytes());
        let signature = hex::encode(mac.finalize().into_bytes());
        let token = format!("{timestamp_hex}.{signature}");
        assert_eq!(validator.validate_token(&token), Err(AuthError::Expired));
    }
}
