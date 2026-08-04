pub mod admin_events;
pub mod automation;
pub mod connect;
mod token_codec;

pub use admin_events::AdminEventAccessTokenManager;
pub use automation::{SessionAutomationAccessTokenClaims, SessionAutomationAccessTokenManager};
pub use connect::{SessionConnectTicketError, SessionConnectTicketManager};

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use uuid::Uuid;

    use super::automation::SessionAutomationAccessTokenError;
    use super::*;
    use crate::auth::AuthenticatedPrincipal;

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
    fn connect_and_automation_credentials_reject_cross_purpose_replay() {
        let root_secret = b"shared-root-secret".to_vec();
        let connect =
            SessionConnectTicketManager::new(root_secret.clone(), Duration::from_secs(300));
        let automation =
            SessionAutomationAccessTokenManager::new(root_secret, Duration::from_secs(300));
        let session_id = Uuid::now_v7();
        let connect_ticket = connect
            .issue_ticket(session_id, &principal())
            .unwrap()
            .token;
        let automation_token = automation
            .issue_token(session_id, &principal())
            .unwrap()
            .token;

        assert_eq!(
            connect.validate_ticket(&automation_token),
            Err(SessionConnectTicketError::WrongPurpose)
        );
        assert_eq!(
            automation.validate_token(&connect_ticket),
            Err(SessionAutomationAccessTokenError::WrongPurpose)
        );
    }
}
