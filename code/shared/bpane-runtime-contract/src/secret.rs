use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use thiserror::Error;

const MAX_SECRET_BYTES: usize = 16 * 1024;

/// A wire-serializable secret whose debug representation is always redacted.
#[derive(Clone, Eq, PartialEq)]
pub struct SecretValue(String);

impl SecretValue {
    /// Creates a bounded, non-empty secret value.
    ///
    /// # Errors
    ///
    /// Returns an error when the value is empty or larger than 16 KiB.
    pub fn new(value: impl Into<String>) -> Result<Self, SecretValueError> {
        let value = value.into();
        if value.is_empty() {
            return Err(SecretValueError::Empty);
        }
        if value.len() > MAX_SECRET_BYTES {
            return Err(SecretValueError::TooLarge);
        }
        Ok(Self(value))
    }

    /// Exposes the secret to the trusted transport or runtime adapter.
    pub fn expose_secret(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SecretValue {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SecretValue([REDACTED])")
    }
}

impl Serialize for SecretValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for SecretValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Validation errors for [`SecretValue`].
#[derive(Debug, Clone, Copy, Eq, Error, PartialEq)]
pub enum SecretValueError {
    /// Empty secrets are invalid.
    #[error("secret value must not be empty")]
    Empty,
    /// Secrets larger than the contract limit are invalid.
    #[error("secret value exceeds the 16 KiB contract limit")]
    TooLarge,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_is_redacted() {
        let secret = SecretValue::new("super-secret-value").unwrap();

        let debug = format!("{secret:?}");

        assert_eq!(debug, "SecretValue([REDACTED])");
        assert!(!debug.contains(secret.expose_secret()));
    }

    #[test]
    fn deserialization_enforces_secret_bounds() {
        assert!(serde_json::from_str::<SecretValue>(r#"""#).is_err());
        let oversized = format!("\"{}\"", "x".repeat(MAX_SECRET_BYTES + 1));
        assert!(serde_json::from_str::<SecretValue>(&oversized).is_err());
    }
}
