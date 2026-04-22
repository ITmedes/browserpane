use crate::auth::{AuthError, AuthValidator};
use crate::connect_ticket::{SessionConnectTicketError, SessionConnectTicketManager};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RequestValidationError {
    MissingToken,
    InvalidToken(AuthError),
    InvalidSessionTicket(SessionConnectTicketError),
}

pub(super) async fn validate_request_path(
    path: &str,
    auth_validator: &AuthValidator,
    connect_ticket_manager: &SessionConnectTicketManager,
) -> Result<(), RequestValidationError> {
    match extract_connect_credential(path) {
        Some(ConnectCredential::SessionTicket(ticket)) => connect_ticket_manager
            .validate_ticket(&ticket)
            .map(|_| ())
            .map_err(RequestValidationError::InvalidSessionTicket),
        Some(ConnectCredential::BearerToken(token)) => auth_validator
            .validate_token(&token)
            .await
            .map_err(RequestValidationError::InvalidToken),
        None => Err(RequestValidationError::MissingToken),
    }
}

enum ConnectCredential {
    SessionTicket(String),
    BearerToken(String),
}

fn extract_connect_credential(path: &str) -> Option<ConnectCredential> {
    let query = path.split('?').nth(1)?;
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("session_ticket=") {
            return Some(ConnectCredential::SessionTicket(value.to_string()));
        }
        if let Some(value) = param.strip_prefix("access_token=") {
            return Some(ConnectCredential::BearerToken(value.to_string()));
        }
        if let Some(value) = param.strip_prefix("token=") {
            return Some(ConnectCredential::BearerToken(value.to_string()));
        }
    }
    None
}

pub(super) fn extract_token(path: &str) -> Option<String> {
    match extract_connect_credential(path)? {
        ConnectCredential::SessionTicket(value) | ConnectCredential::BearerToken(value) => {
            Some(value)
        }
    }
}
