use super::*;

fn owned_identity_mapping_matches(
    mapping: &StoredIdentityMapping,
    principal: &AuthenticatedPrincipal,
) -> bool {
    mapping.owner_subject == principal.subject && mapping.owner_issuer == principal.issuer
}

fn identity_mapping_key_matches(
    mapping: &StoredIdentityMapping,
    request: &PersistIdentityMappingRequest,
) -> bool {
    mapping.kind == request.kind
        && mapping.issuer == request.issuer
        && mapping.external_id == request.external_id
        && mapping.claim_name == request.claim_name
        && mapping.project_id == request.project_id
}

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_identity_mapping(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistIdentityMappingRequest,
    ) -> Result<StoredIdentityMapping, SessionStoreError> {
        let now = Utc::now();
        let mut mappings = self.identity_mappings.lock().await;
        if mappings.iter().any(|mapping| {
            owned_identity_mapping_matches(mapping, principal)
                && identity_mapping_key_matches(mapping, &request)
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "identity mapping for {} {} on project {} already exists",
                request.kind.as_str(),
                request.external_id,
                request.project_id
            )));
        }
        let mapping = StoredIdentityMapping {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            kind: request.kind,
            issuer: request.issuer,
            external_id: request.external_id,
            claim_name: request.claim_name,
            service_principal_id: request.service_principal_id,
            project_id: request.project_id,
            labels: request.labels,
            scopes: request.scopes,
            state: request.state,
            last_seen_at: None,
            created_at: now,
            updated_at: now,
        };
        mappings.push(mapping.clone());
        Ok(mapping)
    }

    pub(in crate::session_control) async fn list_identity_mappings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredIdentityMapping>, SessionStoreError> {
        let mut mappings = self
            .identity_mappings
            .lock()
            .await
            .iter()
            .filter(|mapping| owned_identity_mapping_matches(mapping, principal))
            .cloned()
            .collect::<Vec<_>>();
        mappings.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(mappings)
    }

    pub(in crate::session_control) async fn get_identity_mapping_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredIdentityMapping>, SessionStoreError> {
        Ok(self
            .identity_mappings
            .lock()
            .await
            .iter()
            .find(|mapping| mapping.id == id && owned_identity_mapping_matches(mapping, principal))
            .cloned())
    }

    pub(in crate::session_control) async fn update_identity_mapping_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistIdentityMappingRequest,
    ) -> Result<Option<StoredIdentityMapping>, SessionStoreError> {
        let mut mappings = self.identity_mappings.lock().await;
        if mappings.iter().any(|mapping| {
            mapping.id != id
                && owned_identity_mapping_matches(mapping, principal)
                && identity_mapping_key_matches(mapping, &request)
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "identity mapping for {} {} on project {} already exists",
                request.kind.as_str(),
                request.external_id,
                request.project_id
            )));
        }
        let Some(mapping) = mappings
            .iter_mut()
            .find(|mapping| mapping.id == id && owned_identity_mapping_matches(mapping, principal))
        else {
            return Ok(None);
        };
        mapping.name = request.name;
        mapping.description = request.description;
        mapping.kind = request.kind;
        mapping.issuer = request.issuer;
        mapping.external_id = request.external_id;
        mapping.claim_name = request.claim_name;
        mapping.service_principal_id = request.service_principal_id;
        mapping.project_id = request.project_id;
        mapping.labels = request.labels;
        mapping.scopes = request.scopes;
        mapping.state = request.state;
        mapping.updated_at = Utc::now();
        Ok(Some(mapping.clone()))
    }
}
