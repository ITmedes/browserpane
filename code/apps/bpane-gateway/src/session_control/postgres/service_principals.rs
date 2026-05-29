use super::*;

const SERVICE_PRINCIPAL_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    name,
    description,
    client_id,
    issuer,
    labels,
    scopes,
    allowed_project_ids,
    state,
    last_seen_at,
    last_delegated_at,
    created_at,
    updated_at
"#;

pub(super) struct ServicePrincipalRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn service_principal_repository(&self) -> ServicePrincipalRepository<'_> {
        ServicePrincipalRepository { store: self }
    }

    pub(in crate::session_control) async fn create_service_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistServicePrincipalRequest,
    ) -> Result<StoredServicePrincipal, SessionStoreError> {
        self.service_principal_repository()
            .create_service_principal(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_service_principals_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredServicePrincipal>, SessionStoreError> {
        self.service_principal_repository()
            .list_service_principals_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_service_principal_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        self.service_principal_repository()
            .get_service_principal_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn get_service_principal_for_owner_by_external_identity(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        self.service_principal_repository()
            .get_service_principal_for_owner_by_external_identity(principal, issuer, client_id)
            .await
    }

    pub(in crate::session_control) async fn update_service_principal_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistServicePrincipalRequest,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        self.service_principal_repository()
            .update_service_principal_for_owner(principal, id, request)
            .await
    }

    pub(in crate::session_control) async fn mark_service_principal_seen_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        self.service_principal_repository()
            .mark_service_principal_timestamp(principal, issuer, client_id, "last_seen_at")
            .await
    }

    pub(in crate::session_control) async fn mark_service_principal_delegated_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        self.service_principal_repository()
            .mark_service_principal_timestamp(principal, issuer, client_id, "last_delegated_at")
            .await
    }
}

impl ServicePrincipalRepository<'_> {
    async fn create_service_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistServicePrincipalRequest,
    ) -> Result<StoredServicePrincipal, SessionStoreError> {
        let now = Utc::now();
        let query = format!(
            r#"
            INSERT INTO control_service_principals (
                id,
                owner_subject,
                owner_issuer,
                name,
                description,
                client_id,
                issuer,
                labels,
                scopes,
                allowed_project_ids,
                state,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, $9::jsonb, $10::jsonb, $11, $12, $12)
            RETURNING
                {SERVICE_PRINCIPAL_COLUMNS}
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
                    &request.client_id,
                    &request.issuer,
                    &json_labels(&request.labels),
                    &json_string_array(&request.scopes),
                    &json_uuid_array(&request.allowed_project_ids),
                    &request.state.as_str(),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "service principal {} from issuer {} already exists",
                        request.client_id, request.issuer
                    ));
                }
                SessionStoreError::Backend(format!("failed to create service principal: {error}"))
            })?;
        row_to_stored_service_principal(&row)
    }

    async fn list_service_principals_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredServicePrincipal>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SERVICE_PRINCIPAL_COLUMNS}
            FROM control_service_principals
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
                SessionStoreError::Backend(format!("failed to list service principals: {error}"))
            })?;
        rows.iter().map(row_to_stored_service_principal).collect()
    }

    async fn get_service_principal_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SERVICE_PRINCIPAL_COLUMNS}
            FROM control_service_principals
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
                SessionStoreError::Backend(format!("failed to fetch service principal: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_service_principal)
            .transpose()
    }

    async fn get_service_principal_for_owner_by_external_identity(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SERVICE_PRINCIPAL_COLUMNS}
            FROM control_service_principals
            WHERE owner_subject = $1
              AND owner_issuer = $2
              AND issuer = $3
              AND client_id = $4
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                &query,
                &[&principal.subject, &principal.issuer, &issuer, &client_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to fetch service principal by external identity: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_service_principal)
            .transpose()
    }

    async fn update_service_principal_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistServicePrincipalRequest,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        let query = format!(
            r#"
            UPDATE control_service_principals
            SET
                name = $4,
                description = $5,
                client_id = $6,
                issuer = $7,
                labels = $8::jsonb,
                scopes = $9::jsonb,
                allowed_project_ids = $10::jsonb,
                state = $11,
                updated_at = NOW()
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {SERVICE_PRINCIPAL_COLUMNS}
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
                    &request.client_id,
                    &request.issuer,
                    &json_labels(&request.labels),
                    &json_string_array(&request.scopes),
                    &json_uuid_array(&request.allowed_project_ids),
                    &request.state.as_str(),
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "service principal {} from issuer {} already exists",
                        request.client_id, request.issuer
                    ));
                }
                SessionStoreError::Backend(format!("failed to update service principal: {error}"))
            })?;
        row.as_ref()
            .map(row_to_stored_service_principal)
            .transpose()
    }

    async fn mark_service_principal_timestamp(
        &self,
        principal: &AuthenticatedPrincipal,
        issuer: &str,
        client_id: &str,
        column: &str,
    ) -> Result<Option<StoredServicePrincipal>, SessionStoreError> {
        let query = format!(
            r#"
            UPDATE control_service_principals
            SET
                {column} = NOW(),
                updated_at = NOW()
            WHERE owner_subject = $1
              AND owner_issuer = $2
              AND issuer = $3
              AND client_id = $4
            RETURNING
                {SERVICE_PRINCIPAL_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                &query,
                &[&principal.subject, &principal.issuer, &issuer, &client_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update service principal timestamp: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_service_principal)
            .transpose()
    }
}
