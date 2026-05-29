use super::*;

fn owned_service_principal_matches(
    service_principal: &StoredServicePrincipal,
    principal: &AuthenticatedPrincipal,
) -> bool {
    service_principal.owner_subject == principal.subject
        && service_principal.owner_issuer == principal.issuer
}

fn external_identity_matches(
    service_principal: &StoredServicePrincipal,
    issuer: &str,
    client_id: &str,
) -> bool {
    service_principal.issuer == issuer && service_principal.client_id == client_id
}

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_service_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistServicePrincipalRequest,
    ) -> Result<StoredServicePrincipal, SessionStoreError> {
        let now = Utc::now();
        let mut service_principals = self.service_principals.lock().await;
        if service_principals.iter().any(|service_principal| {
            owned_service_principal_matches(service_principal, principal)
                && external_identity_matches(service_principal, &request.issuer, &request.client_id)
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "service principal {} from issuer {} already exists",
                request.client_id, request.issuer
            )));
        }
        let service_principal = StoredServicePrincipal {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            client_id: request.client_id,
            issuer: request.issuer,
            labels: request.labels,
            scopes: request.scopes,
            allowed_project_ids: request.allowed_project_ids,
            state: request.state,
            last_seen_at: None,
            last_delegated_at: None,
            created_at: now,
            updated_at: now,
        };
        service_principals.push(service_principal.clone());
        Ok(service_principal)
    }

    pub(in crate::session_control) async fn list_service_principals_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredServicePrincipal>, SessionStoreError> {
        let mut service_principals = self
            .service_principals
            .lock()
            .await
            .iter()
            .filter(|service_principal| {
                owned_service_principal_matches(service_principal, principal)
            })
            .cloned()
            .collect::<Vec<_>>();
        service_principals.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(service_principals)
    }

    pub(in crate::session_control) async fn get_service_principal_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        Ok(self
            .service_principals
            .lock()
            .await
            .iter()
            .find(|service_principal| {
                service_principal.id == id
                    && owned_service_principal_matches(service_principal, principal)
            })
            .cloned())
    }

    pub(in crate::session_control) async fn get_service_principal_for_owner_by_external_identity(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        Ok(self
            .service_principals
            .lock()
            .await
            .iter()
            .find(|service_principal| {
                owned_service_principal_matches(service_principal, principal)
                    && external_identity_matches(service_principal, issuer, client_id)
            })
            .cloned())
    }

    pub(in crate::session_control) async fn update_service_principal_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistServicePrincipalRequest,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        let mut service_principals = self.service_principals.lock().await;
        if service_principals.iter().any(|service_principal| {
            service_principal.id != id
                && owned_service_principal_matches(service_principal, principal)
                && external_identity_matches(service_principal, &request.issuer, &request.client_id)
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "service principal {} from issuer {} already exists",
                request.client_id, request.issuer
            )));
        }
        let Some(service_principal) = service_principals.iter_mut().find(|service_principal| {
            service_principal.id == id
                && owned_service_principal_matches(service_principal, principal)
        }) else {
            return Ok(None);
        };
        service_principal.name = request.name;
        service_principal.description = request.description;
        service_principal.client_id = request.client_id;
        service_principal.issuer = request.issuer;
        service_principal.labels = request.labels;
        service_principal.scopes = request.scopes;
        service_principal.allowed_project_ids = request.allowed_project_ids;
        service_principal.state = request.state;
        service_principal.updated_at = Utc::now();
        Ok(Some(service_principal.clone()))
    }

    pub(in crate::session_control) async fn mark_service_principal_seen_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        self.update_service_principal_timestamp(principal, issuer, client_id, true)
            .await
    }

    pub(in crate::session_control) async fn mark_service_principal_delegated_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        self.update_service_principal_timestamp(principal, issuer, client_id, false)
            .await
    }

    async fn update_service_principal_timestamp(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
        seen: bool,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        let mut service_principals = self.service_principals.lock().await;
        let Some(service_principal) = service_principals.iter_mut().find(|service_principal| {
            owned_service_principal_matches(service_principal, principal)
                && external_identity_matches(service_principal, issuer, client_id)
        }) else {
            return Ok(None);
        };
        let now = Utc::now();
        if seen {
            service_principal.last_seen_at = Some(now);
        } else {
            service_principal.last_delegated_at = Some(now);
        }
        service_principal.updated_at = now;
        Ok(Some(service_principal.clone()))
    }
}
