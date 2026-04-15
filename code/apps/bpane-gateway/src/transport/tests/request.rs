use super::*;

#[test]
fn extract_token_from_path() {
    assert_eq!(
        extract_token("/session?token=abc123"),
        Some("abc123".to_string())
    );
    assert_eq!(
        extract_token("/?token=xyz&other=1"),
        Some("xyz".to_string())
    );
    assert_eq!(extract_token("/session"), None);
    assert_eq!(extract_token("/session?other=1"), None);
}

#[test]
fn validate_request_path_accepts_valid_token() {
    let validator = TokenValidator::new(b"transport-request-secret".to_vec());
    let token = validator.generate_token();
    let path = format!("/session?token={token}");

    assert_eq!(validate_request_path(&path, &validator), Ok(()));
}

#[test]
fn validate_request_path_rejects_missing_token() {
    let validator = TokenValidator::new(b"transport-request-secret".to_vec());

    assert_eq!(
        validate_request_path("/session?other=1", &validator),
        Err(RequestValidationError::MissingToken)
    );
}

#[test]
fn validate_request_path_rejects_invalid_token() {
    let validator = TokenValidator::new(b"transport-request-secret".to_vec());

    assert_eq!(
        validate_request_path("/session?token=not-a-token", &validator),
        Err(RequestValidationError::InvalidToken(
            AuthError::MalformedToken
        ))
    );
}
