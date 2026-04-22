use super::*;

#[test]
fn extract_token_from_path() {
    assert_eq!(
        extract_token("/session?token=abc123"),
        Some("abc123".to_string())
    );
    assert_eq!(
        extract_token("/session?access_token=jwt123"),
        Some("jwt123".to_string())
    );
    assert_eq!(
        extract_token("/?token=xyz&other=1"),
        Some("xyz".to_string())
    );
    assert_eq!(extract_token("/session"), None);
    assert_eq!(extract_token("/session?other=1"), None);
}

#[tokio::test]
async fn validate_request_path_accepts_valid_token() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let token = validator.generate_token().unwrap();
    let path = format!("/session?token={token}");

    assert_eq!(validate_request_path(&path, &validator).await, Ok(()));
}

#[tokio::test]
async fn validate_request_path_rejects_missing_token() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());

    assert_eq!(
        validate_request_path("/session?other=1", &validator).await,
        Err(RequestValidationError::MissingToken)
    );
}

#[tokio::test]
async fn validate_request_path_rejects_invalid_token() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());

    assert_eq!(
        validate_request_path("/session?token=not-a-token", &validator).await,
        Err(RequestValidationError::InvalidToken(
            AuthError::MalformedToken
        ))
    );
}
