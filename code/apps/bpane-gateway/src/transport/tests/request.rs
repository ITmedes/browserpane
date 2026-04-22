use super::*;
use std::time::Duration;

use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;
use crate::connect_ticket::{SessionConnectTicketError, SessionConnectTicketManager};

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
        extract_token("/session?session_ticket=ticket123"),
        Some("ticket123".to_string())
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
    let ticket_manager =
        SessionConnectTicketManager::new(b"transport-request-secret".to_vec(), Duration::from_secs(300));
    let token = validator.generate_token().unwrap();
    let path = format!("/session?token={token}");

    assert_eq!(
        validate_request_path(&path, &validator, &ticket_manager).await,
        Ok(())
    );
}

#[tokio::test]
async fn validate_request_path_accepts_valid_session_ticket() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager =
        SessionConnectTicketManager::new(b"transport-request-secret".to_vec(), Duration::from_secs(300));
    let ticket = ticket_manager
        .issue_ticket(Uuid::now_v7(), &AuthenticatedPrincipal {
            subject: "demo".to_string(),
            issuer: "issuer".to_string(),
            display_name: Some("demo".to_string()),
            client_id: None,
        })
        .unwrap()
        .token;
    let path = format!("/session?session_ticket={ticket}");

    assert_eq!(
        validate_request_path(&path, &validator, &ticket_manager).await,
        Ok(())
    );
}

#[tokio::test]
async fn validate_request_path_rejects_missing_token() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager =
        SessionConnectTicketManager::new(b"transport-request-secret".to_vec(), Duration::from_secs(300));

    assert_eq!(
        validate_request_path("/session?other=1", &validator, &ticket_manager).await,
        Err(RequestValidationError::MissingToken)
    );
}

#[tokio::test]
async fn validate_request_path_rejects_invalid_token() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager =
        SessionConnectTicketManager::new(b"transport-request-secret".to_vec(), Duration::from_secs(300));

    assert_eq!(
        validate_request_path("/session?token=not-a-token", &validator, &ticket_manager).await,
        Err(RequestValidationError::InvalidToken(
            AuthError::MalformedToken
        ))
    );
}

#[tokio::test]
async fn validate_request_path_rejects_invalid_session_ticket() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager =
        SessionConnectTicketManager::new(b"transport-request-secret".to_vec(), Duration::from_secs(300));

    assert_eq!(
        validate_request_path("/session?session_ticket=not-a-ticket", &validator, &ticket_manager)
            .await,
        Err(RequestValidationError::InvalidSessionTicket(
            SessionConnectTicketError::Malformed
        ))
    );
}
