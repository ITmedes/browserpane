use uuid::Uuid;

use crate::auth::{AuthError, AuthValidator, AuthenticatedPrincipal};
use crate::connect_ticket::{SessionConnectTicketError, SessionConnectTicketManager};
use crate::session_control::SessionStore;
use crate::session_hub::BrowserClientRole;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RequestValidationError {
    MissingCredential,
    MissingSessionId,
    InvalidToken(AuthError),
    InvalidSessionTicket(SessionConnectTicketError),
    SessionNotVisible,
    SessionLookupFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ValidatedConnectRequest {
    pub session_id: Uuid,
    pub client_role: BrowserClientRole,
}

pub(super) async fn validate_request_path(
    path: &str,
    auth_validator: &AuthValidator,
    connect_ticket_manager: &SessionConnectTicketManager,
    session_store: &SessionStore,
) -> Result<ValidatedConnectRequest, RequestValidationError> {
    let client_role = parsed_client_role(path);
    match extract_connect_request(path)? {
        ConnectRequest::SessionTicket { ticket } => {
            let claims = connect_ticket_manager
                .validate_ticket(&ticket)
                .map_err(RequestValidationError::InvalidSessionTicket)?;
            let principal = AuthenticatedPrincipal {
                subject: claims.subject,
                issuer: claims.issuer,
                display_name: None,
                client_id: claims.client_id,
            };
            let session = session_store
                .get_session_for_principal(&principal, claims.session_id)
                .await
                .map_err(|_| RequestValidationError::SessionLookupFailed)?;
            if session
                .as_ref()
                .is_none_or(|stored| !stored.state.is_runtime_candidate())
            {
                return Err(RequestValidationError::SessionNotVisible);
            }
            Ok(ValidatedConnectRequest {
                session_id: claims.session_id,
                client_role,
            })
        }
        ConnectRequest::BearerToken { token, session_id } => {
            let principal = auth_validator
                .authenticate(&token)
                .await
                .map_err(RequestValidationError::InvalidToken)?;
            let session = session_store
                .get_session_for_principal(&principal, session_id)
                .await
                .map_err(|_| RequestValidationError::SessionLookupFailed)?;
            if session
                .as_ref()
                .is_none_or(|stored| !stored.state.is_runtime_candidate())
            {
                return Err(RequestValidationError::SessionNotVisible);
            }
            Ok(ValidatedConnectRequest {
                session_id,
                client_role,
            })
        }
    }
}

enum ConnectRequest {
    SessionTicket { ticket: String },
    BearerToken { token: String, session_id: Uuid },
}

fn extract_connect_request(path: &str) -> Result<ConnectRequest, RequestValidationError> {
    let query = path
        .split('?')
        .nth(1)
        .ok_or(RequestValidationError::MissingCredential)?;
    let mut session_ticket: Option<String> = None;
    let mut bearer_token: Option<String> = None;
    let mut session_id: Option<Uuid> = None;

    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("session_ticket=") {
            session_ticket = Some(value.to_string());
            continue;
        }
        if let Some(value) = param.strip_prefix("access_token=") {
            bearer_token = Some(value.to_string());
            continue;
        }
        if let Some(value) = param.strip_prefix("token=") {
            bearer_token = Some(value.to_string());
            continue;
        }
        if let Some(value) = param.strip_prefix("session_id=") {
            session_id = Uuid::parse_str(value).ok();
        }
    }

    if let Some(ticket) = session_ticket {
        return Ok(ConnectRequest::SessionTicket { ticket });
    }

    let token = bearer_token.ok_or(RequestValidationError::MissingCredential)?;
    let session_id = session_id.ok_or(RequestValidationError::MissingSessionId)?;
    Ok(ConnectRequest::BearerToken { token, session_id })
}

fn parsed_client_role(path: &str) -> BrowserClientRole {
    let Some(query) = path.split('?').nth(1) else {
        return BrowserClientRole::Interactive;
    };

    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("client_role=") {
            if value.eq_ignore_ascii_case("recorder") {
                return BrowserClientRole::Recorder;
            }
        }
    }

    BrowserClientRole::Interactive
}

#[cfg(test)]
pub(super) fn extract_token(path: &str) -> Option<String> {
    extract_connect_request(path)
        .ok()
        .map(|request| match request {
            ConnectRequest::SessionTicket { ticket } => ticket,
            ConnectRequest::BearerToken { token, .. } => token,
        })
}
