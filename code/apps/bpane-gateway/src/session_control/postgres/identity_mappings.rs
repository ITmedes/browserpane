use super::*;

const IDENTITY_MAPPING_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    name,
    description,
    mapping_kind,
    issuer,
    external_id,
    claim_name,
    service_principal_id,
    project_id,
    labels,
    scopes,
    state,
    last_seen_at,
    created_at,
    updated_at
"#;

pub(super) struct IdentityMappingRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn identity_mapping_repository(&self) -> IdentityMappingRepository<'_> {
        IdentityMappingRepository { store: self }
    }

    pub(in crate::session_control) async fn create_identity_mapping(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistIdentityMappingRequest,
    ) -> Result<StoredIdentityMapping, SessionStoreError> {
        self.identity_mapping_repository()
            .create_identity_mapping(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_identity_mappings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredIdentityMapping>, SessionStoreError> {
        self.identity_mapping_repository()
            .list_identity_mappings_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_identity_mapping_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredIdentityMapping>, SessionStoreError> {
        self.identity_mapping_repository()
            .get_identity_mapping_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn update_identity_mapping_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistIdentityMappingRequest,
    ) -> Result<Option<StoredIdentityMapping>, SessionStoreError> {
        self.identity_mapping_repository()
            .update_identity_mapping_for_owner(principal, id, request)
            .await
    }
}

impl IdentityMappingRepository<'_> {
    async fn create_identity_mapping(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistIdentityMappingRequest,
    ) -> Result<StoredIdentityMapping, SessionStoreError> {
        let now = Utc::now();
        let query = format!(
            r#"
            INSERT INTO control_identity_mappings (
                id,
                owner_subject,
                owner_issuer,
                name,
                description,
                mapping_kind,
                issuer,
                external_id,
                claim_name,
                service_principal_id,
                project_id,
                labels,
                scopes,
                state,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12::jsonb, $13::jsonb, $14, $15, $15)
            RETURNING
                {IDENTITY_MAPPING_COLUMNS}
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
                    &request.kind.as_str(),
                    &request.issuer,
                    &request.external_id,
                    &request.claim_name,
                    &request.service_principal_id,
                    &request.project_id,
                    &json_labels(&request.labels),
                    &json_string_array(&request.scopes),
                    &request.state.as_str(),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "identity mapping for {} {} on project {} already exists",
                        request.kind.as_str(),
                        request.external_id,
                        request.project_id
                    ));
                }
                SessionStoreError::Backend(format!("failed to create identity mapping: {error}"))
            })?;
        row_to_stored_identity_mapping(&row)
    }

    async fn list_identity_mappings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredIdentityMapping>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {IDENTITY_MAPPING_COLUMNS}
            FROM control_identity_mappings
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
                SessionStoreError::Backend(format!("failed to list identity mappings: {error}"))
            })?;
        rows.iter().map(row_to_stored_identity_mapping).collect()
    }

    async fn get_identity_mapping_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredIdentityMapping>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {IDENTITY_MAPPING_COLUMNS}
            FROM control_identity_mappings
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
                SessionStoreError::Backend(format!("failed to fetch identity mapping: {error}"))
            })?;
        row.as_ref().map(row_to_stored_identity_mapping).transpose()
    }

    async fn update_identity_mapping_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistIdentityMappingRequest,
    ) -> Result<Option<StoredIdentityMapping>, SessionStoreError> {
        let query = format!(
            r#"
            UPDATE control_identity_mappings
            SET
                name = $4,
                description = $5,
                mapping_kind = $6,
                issuer = $7,
                external_id = $8,
                claim_name = $9,
                service_principal_id = $10,
                project_id = $11,
                labels = $12::jsonb,
                scopes = $13::jsonb,
                state = $14,
                updated_at = NOW()
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {IDENTITY_MAPPING_COLUMNS}
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
                    &request.kind.as_str(),
                    &request.issuer,
                    &request.external_id,
                    &request.claim_name,
                    &request.service_principal_id,
                    &request.project_id,
                    &json_labels(&request.labels),
                    &json_string_array(&request.scopes),
                    &request.state.as_str(),
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "identity mapping for {} {} on project {} already exists",
                        request.kind.as_str(),
                        request.external_id,
                        request.project_id
                    ));
                }
                SessionStoreError::Backend(format!("failed to update identity mapping: {error}"))
            })?;
        row.as_ref().map(row_to_stored_identity_mapping).transpose()
    }
}
