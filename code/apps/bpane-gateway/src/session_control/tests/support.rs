use super::*;

pub(super) fn principal(subject: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        subject: subject.to_string(),
        issuer: "https://issuer.example".to_string(),
        display_name: Some(subject.to_string()),
        client_id: None,
    }
}

pub(super) fn service_principal(subject: &str, client_id: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        subject: subject.to_string(),
        issuer: "https://issuer.example".to_string(),
        display_name: Some(client_id.to_string()),
        client_id: Some(client_id.to_string()),
    }
}
