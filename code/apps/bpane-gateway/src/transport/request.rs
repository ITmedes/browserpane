use crate::auth::{AuthError, AuthValidator};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RequestValidationError {
    MissingToken,
    InvalidToken(AuthError),
}

pub(super) async fn validate_request_path(
    path: &str,
    auth_validator: &AuthValidator,
) -> Result<(), RequestValidationError> {
    let token = extract_token(path).ok_or(RequestValidationError::MissingToken)?;
    auth_validator
        .validate_token(&token)
        .await
        .map_err(RequestValidationError::InvalidToken)
}

pub(super) fn extract_token(path: &str) -> Option<String> {
    let query = path.split('?').nth(1)?;
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("access_token=") {
            return Some(value.to_string());
        }
        if let Some(value) = param.strip_prefix("token=") {
            return Some(value.to_string());
        }
    }
    None
}
