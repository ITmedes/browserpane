use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_egress_profile(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistEgressProfileRequest,
    ) -> Result<StoredEgressProfile, SessionStoreError> {
        let mut profiles = self.egress_profiles.lock().await;
        if profiles.iter().any(|profile| {
            profile.owner_subject == principal.subject
                && profile.owner_issuer == principal.issuer
                && profile.name == request.name
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "egress profile {} already exists",
                request.name
            )));
        }

        let now = Utc::now();
        let profile = StoredEgressProfile {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            labels: request.labels,
            proxy: request.proxy,
            bypass_rules: request.bypass_rules,
            custom_ca: request.custom_ca,
            state: request.state,
            created_at: now,
            updated_at: now,
        };
        profiles.push(profile.clone());
        Ok(profile)
    }

    pub(in crate::session_control) async fn list_egress_profiles_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredEgressProfile>, SessionStoreError> {
        let mut profiles = self
            .egress_profiles
            .lock()
            .await
            .iter()
            .filter(|profile| {
                profile.owner_subject == principal.subject
                    && profile.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        profiles.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(profiles)
    }

    pub(in crate::session_control) async fn get_egress_profile_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredEgressProfile>, SessionStoreError> {
        Ok(self
            .egress_profiles
            .lock()
            .await
            .iter()
            .find(|profile| {
                profile.id == id
                    && profile.owner_subject == principal.subject
                    && profile.owner_issuer == principal.issuer
            })
            .cloned())
    }
}
