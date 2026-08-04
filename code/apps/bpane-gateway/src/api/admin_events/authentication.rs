use std::time::Duration;

use axum::extract::ws::{close_code, CloseFrame, Message, WebSocket};
use serde::Deserialize;
use tokio::time::timeout;

use crate::auth::AuthenticatedPrincipal;
use crate::session_access::AdminEventAccessTokenManager;

const AUTHENTICATION_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_AUTHENTICATION_MESSAGE_BYTES: usize = 96 * 1024;
const AUTHENTICATE_MESSAGE_TYPE: &str = "admin.authenticate";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AdminEventAuthenticationMessage {
    message_type: String,
    token: String,
}

pub(super) async fn authenticate_socket(
    socket: &mut WebSocket,
    manager: &AdminEventAccessTokenManager,
) -> Option<AuthenticatedPrincipal> {
    let message = match timeout(AUTHENTICATION_TIMEOUT, socket.recv()).await {
        Ok(Some(Ok(Message::Text(message)))) => message,
        _ => {
            reject_authentication(socket).await;
            return None;
        }
    };
    let principal = match parse_authentication_message(message.as_str(), manager) {
        Ok(principal) => principal,
        Err(()) => {
            reject_authentication(socket).await;
            return None;
        }
    };
    let acknowledgement = serde_json::json!({
        "message_type": "admin.authenticated"
    });
    if socket
        .send(Message::Text(acknowledgement.to_string().into()))
        .await
        .is_err()
    {
        return None;
    }
    Some(principal)
}

fn parse_authentication_message(
    payload: &str,
    manager: &AdminEventAccessTokenManager,
) -> Result<AuthenticatedPrincipal, ()> {
    if payload.len() > MAX_AUTHENTICATION_MESSAGE_BYTES {
        return Err(());
    }
    let message: AdminEventAuthenticationMessage = serde_json::from_str(payload).map_err(|_| ())?;
    if message.message_type != AUTHENTICATE_MESSAGE_TYPE {
        return Err(());
    }
    manager
        .validate_token(&message.token)
        .map(|claims| claims.into_principal())
        .map_err(|_| ())
}

async fn reject_authentication(socket: &mut WebSocket) {
    let _ = socket
        .send(Message::Close(Some(CloseFrame {
            code: close_code::POLICY,
            reason: "authentication failed".into(),
        })))
        .await;
}

#[cfg(test)]
mod tests {
    use crate::auth::AuthenticatedPrincipal;
    use crate::session_access::SessionConnectTicketManager;
    use uuid::Uuid;

    use super::*;

    fn principal() -> AuthenticatedPrincipal {
        AuthenticatedPrincipal {
            subject: "operator".to_string(),
            issuer: "issuer".to_string(),
            display_name: Some("Operator".to_string()),
            client_id: None,
            safe_claims: Default::default(),
        }
    }

    #[test]
    fn accepts_only_the_scoped_authentication_message() {
        let manager = AdminEventAccessTokenManager::new(vec![8; 32], Duration::from_secs(60));
        let token = manager.issue_token(&principal()).unwrap().token;
        let payload = serde_json::json!({
            "message_type": "admin.authenticate",
            "token": token
        });

        assert_eq!(
            parse_authentication_message(&payload.to_string(), &manager).unwrap(),
            principal()
        );
    }

    #[test]
    fn rejects_wrong_message_shape_and_cross_purpose_tokens() {
        let secret = vec![8; 32];
        let manager = AdminEventAccessTokenManager::new(secret.clone(), Duration::from_secs(60));
        let connect = SessionConnectTicketManager::new(secret, Duration::from_secs(60));
        let connect_token = connect
            .issue_ticket(Uuid::now_v7(), &principal())
            .unwrap()
            .token;

        assert_eq!(
            parse_authentication_message(
                &serde_json::json!({
                    "message_type": "admin.authenticate",
                    "token": connect_token
                })
                .to_string(),
                &manager
            ),
            Err(())
        );
        assert_eq!(
            parse_authentication_message(
                r#"{"message_type":"admin.authenticate","token":"token","extra":true}"#,
                &manager
            ),
            Err(())
        );
        assert_eq!(
            parse_authentication_message(
                r#"{"message_type":"unsupported","token":"token"}"#,
                &manager
            ),
            Err(())
        );
    }

    #[test]
    fn rejects_oversized_messages_before_parsing() {
        let manager = AdminEventAccessTokenManager::new(vec![8; 32], Duration::from_secs(60));
        let oversized = "x".repeat(MAX_AUTHENTICATION_MESSAGE_BYTES + 1);

        assert_eq!(parse_authentication_message(&oversized, &manager), Err(()));
    }
}
