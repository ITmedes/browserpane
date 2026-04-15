use crate::auth::{AuthError, TokenValidator};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RequestValidationError {
    MissingToken,
    InvalidToken(AuthError),
}

pub(super) fn validate_request_path(
    path: &str,
    token_validator: &TokenValidator,
) -> Result<(), RequestValidationError> {
    let token = extract_token(path).ok_or(RequestValidationError::MissingToken)?;
    token_validator
        .validate_token(&token)
        .map_err(RequestValidationError::InvalidToken)
}

pub(super) fn extract_token(path: &str) -> Option<String> {
    let query = path.split('?').nth(1)?;
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("token=") {
            return Some(value.to_string());
        }
    }
    None
}
