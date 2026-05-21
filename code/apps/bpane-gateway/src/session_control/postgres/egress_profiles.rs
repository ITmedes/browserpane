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
                state,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::jsonb, $8::jsonb, $9::jsonb, $10, $11, $11)
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
}
