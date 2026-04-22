use super::*;
use std::collections::HashMap;
use std::time::Duration;

use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;
use crate::connect_ticket::{SessionConnectTicketError, SessionConnectTicketManager};
use crate::session_control::{CreateSessionRequest, SessionOwnerMode, SessionStore};

fn empty_request() -> CreateSessionRequest {
    CreateSessionRequest {
        template_id: None,
        owner_mode: None,
        viewport: None,
        idle_timeout_sec: None,
        labels: HashMap::new(),
        integration_context: None,
    }
}

#[test]
fn extract_token_from_path() {
    assert_eq!(
        extract_token("/session?token=abc123&session_id=0195bf4d-d091-7000-8000-000000000001"),
        Some("abc123".to_string())
    );
    assert_eq!(
        extract_token(
            "/session?access_token=jwt123&session_id=0195bf4d-d091-7000-8000-000000000001"
        ),
        Some("jwt123".to_string())
    );
    assert_eq!(
        extract_token("/session?session_ticket=ticket123"),
        Some("ticket123".to_string())
    );
    assert_eq!(
        extract_token("/?token=xyz&other=1&session_id=0195bf4d-d091-7000-8000-000000000001"),
        Some("xyz".to_string())
    );
    assert_eq!(extract_token("/session"), None);
    assert_eq!(extract_token("/session?other=1"), None);
}

#[tokio::test]
async fn validate_request_path_accepts_valid_token() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager = SessionConnectTicketManager::new(
        b"transport-request-secret".to_vec(),
        Duration::from_secs(300),
    );
    let token = validator.generate_token().unwrap();
    let store = SessionStore::in_memory();
    let principal = validator.authenticate(&token).await.unwrap();
    let session = store
        .create_session(&principal, empty_request(), SessionOwnerMode::Collaborative)
        .await
        .unwrap();
    let path = format!("/session?token={token}&session_id={}", session.id);

    assert_eq!(
        validate_request_path(&path, &validator, &ticket_manager, &store).await,
        Ok(ValidatedConnectRequest {
            session_id: session.id
        })
    );
}

#[tokio::test]
async fn validate_request_path_accepts_valid_session_ticket() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager = SessionConnectTicketManager::new(
        b"transport-request-secret".to_vec(),
        Duration::from_secs(300),
    );
    let store = SessionStore::in_memory();
    let principal = AuthenticatedPrincipal {
        subject: "demo".to_string(),
        issuer: "issuer".to_string(),
        display_name: Some("demo".to_string()),
        client_id: None,
    };
    let session = store
        .create_session(&principal, empty_request(), SessionOwnerMode::Collaborative)
        .await
        .unwrap();
    let ticket = ticket_manager
        .issue_ticket(session.id, &principal)
        .unwrap()
        .token;
    let path = format!("/session?session_ticket={ticket}");

    assert_eq!(
        validate_request_path(&path, &validator, &ticket_manager, &store).await,
        Ok(ValidatedConnectRequest {
            session_id: session.id
        })
    );
}

#[tokio::test]
async fn validate_request_path_rejects_missing_token() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager = SessionConnectTicketManager::new(
        b"transport-request-secret".to_vec(),
        Duration::from_secs(300),
    );
    let store = SessionStore::in_memory();

    assert_eq!(
        validate_request_path("/session?other=1", &validator, &ticket_manager, &store).await,
        Err(RequestValidationError::MissingCredential)
    );
}

#[tokio::test]
async fn validate_request_path_rejects_invalid_token() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager = SessionConnectTicketManager::new(
        b"transport-request-secret".to_vec(),
        Duration::from_secs(300),
    );
    let store = SessionStore::in_memory();
    let session_id = Uuid::now_v7();

    assert_eq!(
        validate_request_path(
            &format!("/session?token=not-a-token&session_id={session_id}"),
            &validator,
            &ticket_manager,
            &store
        )
        .await,
        Err(RequestValidationError::InvalidToken(
            AuthError::MalformedToken
        ))
    );
}

#[tokio::test]
async fn validate_request_path_rejects_invalid_session_ticket() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager = SessionConnectTicketManager::new(
        b"transport-request-secret".to_vec(),
        Duration::from_secs(300),
    );
    let store = SessionStore::in_memory();

    assert_eq!(
        validate_request_path(
            "/session?session_ticket=not-a-ticket",
            &validator,
            &ticket_manager,
            &store
        )
        .await,
        Err(RequestValidationError::InvalidSessionTicket(
            SessionConnectTicketError::Malformed
        ))
    );
}

#[tokio::test]
async fn validate_request_path_rejects_missing_session_id_for_bearer_connect() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager = SessionConnectTicketManager::new(
        b"transport-request-secret".to_vec(),
        Duration::from_secs(300),
    );
    let store = SessionStore::in_memory();
    let token = validator.generate_token().unwrap();

    assert_eq!(
        validate_request_path(
            &format!("/session?token={token}"),
            &validator,
            &ticket_manager,
            &store
        )
        .await,
        Err(RequestValidationError::MissingSessionId)
    );
}

#[tokio::test]
async fn validate_request_path_rejects_session_not_visible() {
    let validator = AuthValidator::from_hmac_secret(b"transport-request-secret".to_vec());
    let ticket_manager = SessionConnectTicketManager::new(
        b"transport-request-secret".to_vec(),
        Duration::from_secs(300),
    );
    let store = SessionStore::in_memory();
    let token = validator.generate_token().unwrap();
    let principal = validator.authenticate(&token).await.unwrap();
    let other_principal = AuthenticatedPrincipal {
        subject: "other".to_string(),
        issuer: principal.issuer.clone(),
        display_name: None,
        client_id: None,
    };
    let session = store
        .create_session(
            &other_principal,
            empty_request(),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    assert_eq!(
        validate_request_path(
            &format!("/session?token={token}&session_id={}", session.id),
            &validator,
            &ticket_manager,
            &store
        )
        .await,
        Err(RequestValidationError::SessionNotVisible)
    );
}
