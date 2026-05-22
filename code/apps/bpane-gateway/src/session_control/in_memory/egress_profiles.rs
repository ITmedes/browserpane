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
            traffic_observation: request.traffic_observation,
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

    pub(in crate::session_control) async fn update_egress_profile_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistEgressProfileRequest,
    ) -> Result<Option<StoredEgressProfile>, SessionStoreError> {
        let mut profiles = self.egress_profiles.lock().await;
        if profiles.iter().any(|profile| {
            profile.id != id
                && profile.owner_subject == principal.subject
                && profile.owner_issuer == principal.issuer
                && profile.name == request.name
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "egress profile {} already exists",
                request.name
            )));
        }
        let Some(profile) = profiles.iter_mut().find(|profile| {
            profile.id == id
                && profile.owner_subject == principal.subject
                && profile.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        profile.name = request.name;
        profile.description = request.description;
        profile.labels = request.labels;
        profile.proxy = request.proxy;
        profile.bypass_rules = request.bypass_rules;
        profile.custom_ca = request.custom_ca;
        profile.traffic_observation = request.traffic_observation;
        profile.state = request.state;
        profile.updated_at = Utc::now();
        Ok(Some(profile.clone()))
    }

    pub(in crate::session_control) async fn upsert_egress_diagnostics_probe_result(
        &self,
        result: PersistEgressDiagnosticsProbeResult,
    ) -> Result<StoredEgressDiagnosticsProbeResult, SessionStoreError> {
        let mut results = self.egress_diagnostics_probe_results.lock().await;
        results.insert(result.session_id, result.clone());
        Ok(result)
    }

    pub(in crate::session_control) async fn get_egress_diagnostics_probe_result_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredEgressDiagnosticsProbeResult>, SessionStoreError> {
        Ok(self
            .egress_diagnostics_probe_results
            .lock()
            .await
            .get(&session_id)
            .cloned())
    }

    pub(in crate::session_control) async fn upsert_egress_profile_reachability_probe_result(
        &self,
        result: PersistEgressProfileReachabilityProbeResult,
    ) -> Result<StoredEgressProfileReachabilityProbeResult, SessionStoreError> {
        let mut results = self.egress_profile_reachability_probe_results.lock().await;
        results.insert(result.profile_id, result.clone());
        Ok(result)
    }

    pub(in crate::session_control) async fn get_egress_profile_reachability_probe_result(
        &self,
        profile_id: Uuid,
    ) -> Result<Option<StoredEgressProfileReachabilityProbeResult>, SessionStoreError> {
        Ok(self
            .egress_profile_reachability_probe_results
            .lock()
            .await
            .get(&profile_id)
            .cloned())
    }

    pub(in crate::session_control) async fn list_egress_profile_reachability_probe_results_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<HashMap<Uuid, StoredEgressProfileReachabilityProbeResult>, SessionStoreError> {
        let profile_ids = self
            .egress_profiles
            .lock()
            .await
            .iter()
            .filter(|profile| {
                profile.owner_subject == principal.subject
                    && profile.owner_issuer == principal.issuer
            })
            .map(|profile| profile.id)
            .collect::<Vec<_>>();
        let results = self.egress_profile_reachability_probe_results.lock().await;
        Ok(profile_ids
            .into_iter()
            .filter_map(|profile_id| {
                results
                    .get(&profile_id)
                    .cloned()
                    .map(|result| (profile_id, result))
            })
            .collect())
    }
}
