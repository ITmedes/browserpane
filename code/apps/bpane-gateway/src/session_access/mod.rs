pub mod admin_events;
pub mod automation;
pub mod connect;
pub mod recording_worker;
mod token_codec;

pub use admin_events::AdminEventAccessTokenManager;
pub use automation::{SessionAutomationAccessTokenClaims, SessionAutomationAccessTokenManager};
pub use connect::{SessionConnectTicketError, SessionConnectTicketManager};
pub use recording_worker::RecordingWorkerAccessTokenManager;

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use uuid::Uuid;

    use super::admin_events::AdminEventAccessTokenError;
    use super::automation::SessionAutomationAccessTokenError;
    use super::recording_worker::RecordingWorkerAccessTokenError;
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
    fn all_credentials_reject_cross_purpose_replay() {
        let root_secret = b"shared-root-secret".to_vec();
        let connect =
            SessionConnectTicketManager::new(root_secret.clone(), Duration::from_secs(300));
        let automation =
            SessionAutomationAccessTokenManager::new(root_secret.clone(), Duration::from_secs(300));
        let admin_events = AdminEventAccessTokenManager::new(root_secret, Duration::from_secs(60));
        let session_id = Uuid::now_v7();
        let recording_id = Uuid::now_v7();
        let connect_ticket = connect
            .issue_ticket(session_id, &principal())
            .unwrap()
            .token;
        let automation_token = automation
            .issue_token(session_id, &principal())
            .unwrap()
            .token;
        let admin_event_token = admin_events.issue_token(&principal()).unwrap().token;
        let recording_worker = RecordingWorkerAccessTokenManager::new(b"shared-root-secret");
        let recording_worker_token = recording_worker
            .issue_token(session_id, recording_id)
            .unwrap()
            .token;

        assert_eq!(
            connect.validate_ticket(&automation_token),
            Err(SessionConnectTicketError::WrongPurpose)
        );
        assert_eq!(
            connect.validate_ticket(&admin_event_token),
            Err(SessionConnectTicketError::WrongPurpose)
        );
        assert_eq!(
            automation.validate_token(&connect_ticket),
            Err(SessionAutomationAccessTokenError::WrongPurpose)
        );
        assert_eq!(
            automation.validate_token(&admin_event_token),
            Err(SessionAutomationAccessTokenError::WrongPurpose)
        );
        assert_eq!(
            admin_events.validate_token(&connect_ticket),
            Err(AdminEventAccessTokenError::WrongPurpose)
        );
        assert_eq!(
            admin_events.validate_token(&automation_token),
            Err(AdminEventAccessTokenError::WrongPurpose)
        );
        assert_eq!(
            recording_worker.validate_token(&connect_ticket),
            Err(RecordingWorkerAccessTokenError::WrongPurpose)
        );
        assert_eq!(
            recording_worker.validate_token(&automation_token),
            Err(RecordingWorkerAccessTokenError::WrongPurpose)
        );
        assert_eq!(
            recording_worker.validate_token(&admin_event_token),
            Err(RecordingWorkerAccessTokenError::WrongPurpose)
        );
        assert_eq!(
            connect.validate_ticket(&recording_worker_token),
            Err(SessionConnectTicketError::WrongPurpose)
        );
        assert_eq!(
            automation.validate_token(&recording_worker_token),
            Err(SessionAutomationAccessTokenError::WrongPurpose)
        );
        assert_eq!(
            admin_events.validate_token(&recording_worker_token),
            Err(AdminEventAccessTokenError::WrongPurpose)
        );
    }
}
