use super::*;

const EGRESS_PROFILE_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    name,
    description,
    labels,
    proxy,
    bypass_rules,
    custom_ca,
    traffic_observation,
    state,
    created_at,
    updated_at
"#;

pub(super) struct EgressProfileRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn egress_profile_repository(&self) -> EgressProfileRepository<'_> {
        EgressProfileRepository { store: self }
    }

    pub(in crate::session_control) async fn create_egress_profile(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistEgressProfileRequest,
    ) -> Result<StoredEgressProfile, SessionStoreError> {
        self.egress_profile_repository()
            .create_egress_profile(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_egress_profiles_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredEgressProfile>, SessionStoreError> {
        self.egress_profile_repository()
            .list_egress_profiles_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_egress_profile_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredEgressProfile>, SessionStoreError> {
        self.egress_profile_repository()
            .get_egress_profile_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn update_egress_profile_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistEgressProfileRequest,
    ) -> Result<Option<StoredEgressProfile>, SessionStoreError> {
        self.egress_profile_repository()
            .update_egress_profile_for_owner(principal, id, request)
            .await
    }

    pub(in crate::session_control) async fn upsert_egress_diagnostics_probe_result(
        &self,
        result: PersistEgressDiagnosticsProbeResult,
    ) -> Result<StoredEgressDiagnosticsProbeResult, SessionStoreError> {
        self.egress_profile_repository()
            .upsert_egress_diagnostics_probe_result(result)
            .await
    }

    pub(in crate::session_control) async fn get_egress_diagnostics_probe_result_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredEgressDiagnosticsProbeResult>, SessionStoreError> {
        self.egress_profile_repository()
            .get_egress_diagnostics_probe_result_for_session(session_id)
            .await
    }

    pub(in crate::session_control) async fn upsert_egress_profile_reachability_probe_result(
        &self,
        result: PersistEgressProfileReachabilityProbeResult,
    ) -> Result<StoredEgressProfileReachabilityProbeResult, SessionStoreError> {
        self.egress_profile_repository()
            .upsert_egress_profile_reachability_probe_result(result)
            .await
    }

    pub(in crate::session_control) async fn get_egress_profile_reachability_probe_result(
        &self,
        profile_id: Uuid,
    ) -> Result<Option<StoredEgressProfileReachabilityProbeResult>, SessionStoreError> {
        self.egress_profile_repository()
            .get_egress_profile_reachability_probe_result(profile_id)
            .await
    }

    pub(in crate::session_control) async fn list_egress_profile_reachability_probe_results_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<HashMap<Uuid, StoredEgressProfileReachabilityProbeResult>, SessionStoreError> {
        self.egress_profile_repository()
            .list_egress_profile_reachability_probe_results_for_owner(principal)
            .await
    }
}

impl EgressProfileRepository<'_> {
    async fn create_egress_profile(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistEgressProfileRequest,
    ) -> Result<StoredEgressProfile, SessionStoreError> {
        let now = Utc::now();
        let proxy_value = request
            .proxy
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to encode egress proxy: {error}"))
            })?;
        let custom_ca_value = request
            .custom_ca
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to encode egress custom_ca: {error}"))
            })?;
        let traffic_observation_value = serde_json::to_value(&request.traffic_observation)
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to encode egress traffic_observation: {error}"
                ))
            })?;
        let query = format!(
            r#"
            INSERT INTO control_egress_profiles (
                id,
                owner_subject,
                owner_issuer,
                name,
                description,
                labels,
                proxy,
                bypass_rules,
                custom_ca,
                traffic_observation,
                state,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::jsonb, $8::jsonb, $9::jsonb, $10::jsonb, $11, $12, $12)
            RETURNING
                {EGRESS_PROFILE_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_one(
                &query,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.description,
                    &json_labels(&request.labels),
                    &proxy_value,
                    &json_string_array(&request.bypass_rules),
                    &custom_ca_value,
                    &traffic_observation_value,
                    &request.state.as_str(),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "egress profile {} already exists",
                        request.name
                    ));
                }
                SessionStoreError::Backend(format!("failed to create egress profile: {error}"))
            })?;
        row_to_stored_egress_profile(&row)
    }

    async fn list_egress_profiles_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredEgressProfile>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {EGRESS_PROFILE_COLUMNS}
            FROM control_egress_profiles
            WHERE owner_subject = $1
              AND owner_issuer = $2
            ORDER BY created_at DESC
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&query, &[&principal.subject, &principal.issuer])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list egress profiles: {error}"))
            })?;
        rows.iter().map(row_to_stored_egress_profile).collect()
    }

    async fn get_egress_profile_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredEgressProfile>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {EGRESS_PROFILE_COLUMNS}
            FROM control_egress_profiles
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&id, &principal.subject, &principal.issuer])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch egress profile: {error}"))
            })?;
        row.as_ref().map(row_to_stored_egress_profile).transpose()
    }

    async fn update_egress_profile_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistEgressProfileRequest,
    ) -> Result<Option<StoredEgressProfile>, SessionStoreError> {
        let proxy_value = request
            .proxy
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to encode egress proxy: {error}"))
            })?;
        let custom_ca_value = request
            .custom_ca
            .as_ref()
            .map(serde_json::to_value)
            .transpose()
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to encode egress custom_ca: {error}"))
            })?;
        let traffic_observation_value = serde_json::to_value(&request.traffic_observation)
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to encode egress traffic_observation: {error}"
                ))
            })?;
        let query = format!(
            r#"
            UPDATE control_egress_profiles
            SET
                name = $4,
                description = $5,
                labels = $6::jsonb,
                proxy = $7::jsonb,
                bypass_rules = $8::jsonb,
                custom_ca = $9::jsonb,
                traffic_observation = $10::jsonb,
                state = $11,
                updated_at = NOW()
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {EGRESS_PROFILE_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                &query,
                &[
                    &id,
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.description,
                    &json_labels(&request.labels),
                    &proxy_value,
                    &json_string_array(&request.bypass_rules),
                    &custom_ca_value,
                    &traffic_observation_value,
                    &request.state.as_str(),
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "egress profile {} already exists",
                        request.name
                    ));
                }
                SessionStoreError::Backend(format!("failed to update egress profile: {error}"))
            })?;
        row.as_ref().map(row_to_stored_egress_profile).transpose()
    }

    async fn upsert_egress_diagnostics_probe_result(
        &self,
        result: PersistEgressDiagnosticsProbeResult,
    ) -> Result<StoredEgressDiagnosticsProbeResult, SessionStoreError> {
        self.store
            .db
            .client()
            .await?
            .execute(
                r#"
                INSERT INTO control_session_egress_diagnostics_probe_results (
                    session_id,
                    profile_id,
                    active_probe_collected,
                    observed_public_ip,
                    observed_tls_issuer,
                    last_failure_reason,
                    observed_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                ON CONFLICT (session_id) DO UPDATE
                SET
                    profile_id = EXCLUDED.profile_id,
                    active_probe_collected = EXCLUDED.active_probe_collected,
                    observed_public_ip = EXCLUDED.observed_public_ip,
                    observed_tls_issuer = EXCLUDED.observed_tls_issuer,
                    last_failure_reason = EXCLUDED.last_failure_reason,
                    observed_at = EXCLUDED.observed_at
                "#,
                &[
                    &result.session_id,
                    &result.profile_id,
                    &result.active_probe_collected,
                    &result.observed_public_ip,
                    &result.observed_tls_issuer,
                    &result.last_failure_reason,
                    &result.observed_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to upsert egress diagnostics probe result: {error}"
                ))
            })?;
        Ok(result)
    }

    async fn get_egress_diagnostics_probe_result_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<StoredEgressDiagnosticsProbeResult>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    session_id,
                    profile_id,
                    active_probe_collected,
                    observed_public_ip,
                    observed_tls_issuer,
                    last_failure_reason,
                    observed_at
                FROM control_session_egress_diagnostics_probe_results
                WHERE session_id = $1
                "#,
                &[&session_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to fetch egress diagnostics probe result: {error}"
                ))
            })?;
        Ok(row.map(|row| StoredEgressDiagnosticsProbeResult {
            session_id: row.get("session_id"),
            profile_id: row.get("profile_id"),
            active_probe_collected: row.get("active_probe_collected"),
            observed_public_ip: row.get("observed_public_ip"),
            observed_tls_issuer: row.get("observed_tls_issuer"),
            last_failure_reason: row.get("last_failure_reason"),
            observed_at: row.get("observed_at"),
        }))
    }

    async fn upsert_egress_profile_reachability_probe_result(
        &self,
        result: PersistEgressProfileReachabilityProbeResult,
    ) -> Result<StoredEgressProfileReachabilityProbeResult, SessionStoreError> {
        self.store
            .db
            .client()
            .await?
            .execute(
                r#"
                INSERT INTO control_egress_profile_reachability_probe_results (
                    profile_id,
                    reachability_collected,
                    reachability_healthy,
                    last_failure_reason,
                    observed_at
                )
                VALUES ($1, $2, $3, $4, $5)
                ON CONFLICT (profile_id) DO UPDATE
                SET
                    reachability_collected = EXCLUDED.reachability_collected,
                    reachability_healthy = EXCLUDED.reachability_healthy,
                    last_failure_reason = EXCLUDED.last_failure_reason,
                    observed_at = EXCLUDED.observed_at
                "#,
                &[
                    &result.profile_id,
                    &result.reachability_collected,
                    &result.reachability_healthy,
                    &result.last_failure_reason,
                    &result.observed_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to upsert egress profile reachability probe result: {error}"
                ))
            })?;
        Ok(result)
    }

    async fn get_egress_profile_reachability_probe_result(
        &self,
        profile_id: Uuid,
    ) -> Result<Option<StoredEgressProfileReachabilityProbeResult>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    profile_id,
                    reachability_collected,
                    reachability_healthy,
                    last_failure_reason,
                    observed_at
                FROM control_egress_profile_reachability_probe_results
                WHERE profile_id = $1
                "#,
                &[&profile_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to fetch egress profile reachability probe result: {error}"
                ))
            })?;
        Ok(row
            .as_ref()
            .map(row_to_egress_profile_reachability_probe_result))
    }

    async fn list_egress_profile_reachability_probe_results_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<HashMap<Uuid, StoredEgressProfileReachabilityProbeResult>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    result.profile_id,
                    result.reachability_collected,
                    result.reachability_healthy,
                    result.last_failure_reason,
                    result.observed_at
                FROM control_egress_profile_reachability_probe_results result
                JOIN control_egress_profiles profile ON profile.id = result.profile_id
                WHERE profile.owner_subject = $1
                  AND profile.owner_issuer = $2
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list egress profile reachability probe results: {error}"
                ))
            })?;
        Ok(rows
            .iter()
            .map(row_to_egress_profile_reachability_probe_result)
            .map(|result| (result.profile_id, result))
            .collect())
    }
}

fn row_to_egress_profile_reachability_probe_result(
    row: &Row,
) -> StoredEgressProfileReachabilityProbeResult {
    StoredEgressProfileReachabilityProbeResult {
        profile_id: row.get("profile_id"),
        reachability_collected: row.get("reachability_collected"),
        reachability_healthy: row.get("reachability_healthy"),
        last_failure_reason: row.get("last_failure_reason"),
        observed_at: row.get("observed_at"),
    }
}
