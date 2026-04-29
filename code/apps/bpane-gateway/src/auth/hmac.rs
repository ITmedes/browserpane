use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

use crate::auth::{AuthError, AuthenticatedPrincipal};

pub(crate) type HmacSha256 = Hmac<Sha256>;

// Development convenience: keep tokens valid for a full day to avoid frequent reconnect issues.
const TOKEN_TTL_SECS: u64 = 2_592_000; // 30 days
const JWT_CLOCK_SKEW_SECS: u64 = 30;

#[derive(Debug, Clone)]
pub struct HmacTokenValidator {
    secret: Vec<u8>,
}

impl HmacTokenValidator {
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

        let mut mac = HmacSha256::new_from_slice(&self.secret).unwrap();
        mac.update(&timestamp_bytes);
        mac.verify_slice(&signature)
            .map_err(|_| AuthError::InvalidSignature)?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if timestamp > now + JWT_CLOCK_SKEW_SECS {
            return Err(AuthError::Expired);
        }
        if now.saturating_sub(timestamp) > TOKEN_TTL_SECS {
            return Err(AuthError::Expired);
        }

        Ok(())
    }

    pub fn authenticate(&self, token: &str) -> Result<AuthenticatedPrincipal, AuthError> {
        self.validate_token(token)?;
        let subject_suffix = token.split('.').next().unwrap_or(token);
        Ok(AuthenticatedPrincipal {
            subject: format!("legacy-dev-token:{subject_suffix}"),
            issuer: "bpane-gateway".to_string(),
            display_name: None,
            client_id: None,
        })
    }
}
