use std::time::{SystemTime, UNIX_EPOCH};

use hmac::Mac;

use super::{AuthError, AuthValidator, HmacSha256, HmacTokenValidator};

#[test]
fn generate_and_validate_token() {
    let validator = HmacTokenValidator::new(b"test-secret-key".to_vec());
    let token = validator.generate_token();
    assert!(validator.validate_token(&token).is_ok());
}

#[test]
fn auth_validator_hmac_mode_generates_token() {
    let validator = AuthValidator::from_hmac_secret(b"test-secret-key".to_vec());
    assert!(validator.generate_token().is_some());
    assert!(!validator.is_oidc());
}

#[test]
fn reject_tampered_token() {
    let validator = HmacTokenValidator::new(b"test-secret-key".to_vec());
    let token = validator.generate_token();
    let mut tampered = token.clone();
    tampered.push('a');
    assert_eq!(
        validator.validate_token(&tampered),
        Err(AuthError::MalformedToken)
    );
}

#[test]
fn reject_wrong_secret() {
    let v1 = HmacTokenValidator::new(b"secret-1".to_vec());
    let v2 = HmacTokenValidator::new(b"secret-2".to_vec());
    let token = v1.generate_token();
    assert_eq!(v2.validate_token(&token), Err(AuthError::InvalidSignature));
}

#[test]
fn reject_malformed() {
    let validator = HmacTokenValidator::new(b"secret".to_vec());
    assert_eq!(
        validator.validate_token("not-a-token"),
        Err(AuthError::MalformedToken)
    );
    assert_eq!(validator.validate_token(""), Err(AuthError::MalformedToken));
}

#[test]
fn reject_empty_parts() {
    let validator = HmacTokenValidator::new(b"secret".to_vec());
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
    let validator = HmacTokenValidator::new(b"secret".to_vec());
    assert_eq!(
        validator.validate_token("aabb.ccdd.eeff"),
        Err(AuthError::MalformedToken)
    );
}

#[test]
fn reject_non_hex_characters() {
    let validator = HmacTokenValidator::new(b"secret".to_vec());
    assert_eq!(
        validator.validate_token("zzzzzzzzzzzzzzzz.abcdef1234567890"),
        Err(AuthError::MalformedToken)
    );
}

#[test]
fn token_within_clock_skew_tolerance_accepted() {
    let validator = HmacTokenValidator::new(b"test-secret-key".to_vec());
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
    assert!(validator.validate_token(&token).is_ok());
}

#[test]
fn different_secrets_produce_different_tokens() {
    let v1 = HmacTokenValidator::new(b"secret-a".to_vec());
    let v2 = HmacTokenValidator::new(b"secret-b".to_vec());
    let t1 = v1.generate_token();
    let t2 = v2.generate_token();
    let sig1 = t1.split('.').nth(1).unwrap();
    let sig2 = t2.split('.').nth(1).unwrap();
    assert_ne!(sig1, sig2);
}

#[test]
fn reject_future_timestamp() {
    let validator = HmacTokenValidator::new(b"test-secret-key".to_vec());
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
