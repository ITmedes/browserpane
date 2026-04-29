use super::*;

pub(super) struct CredentialBindingRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn credential_binding_repository(&self) -> CredentialBindingRepository<'_> {
        CredentialBindingRepository { store: self }
    }

    pub(in crate::session_control) async fn create_credential_binding(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistCredentialBindingRequest,
    ) -> Result<StoredCredentialBinding, SessionStoreError> {
        self.credential_binding_repository()
            .create_credential_binding(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_credential_bindings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredCredentialBinding>, SessionStoreError> {
        self.credential_binding_repository()
            .list_credential_bindings_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_credential_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredCredentialBinding>, SessionStoreError> {
        self.credential_binding_repository()
            .get_credential_binding_for_owner(principal, id)
            .await
    }
}

impl CredentialBindingRepository<'_> {
    pub(in crate::session_control) async fn create_credential_binding(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistCredentialBindingRequest,
    ) -> Result<StoredCredentialBinding, SessionStoreError> {
        let now = Utc::now();
        let totp = request
            .totp
            .as_ref()
            .map(|totp| {
                serde_json::to_value(totp).map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to encode credential binding totp metadata: {error}"
                    ))
                })
            })
            .transpose()?;
        let row = self
            .store
            .db
            .client()
            .await?
            .query_one(
                r#"
                INSERT INTO control_credential_bindings (
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    provider,
                    external_ref,
                    namespace,
                    allowed_origins,
                    injection_mode,
                    totp,
                    labels,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8::jsonb, $9, $10::jsonb, $11::jsonb, $12, $12)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    provider,
                    external_ref,
                    namespace,
                    allowed_origins,
                    injection_mode,
                    totp,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[
                    &request.id,
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.provider.as_str(),
                    &request.external_ref,
                    &request.namespace,
                    &json_string_array(&request.allowed_origins),
                    &request.injection_mode.as_str(),
                    &totp,
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to create credential binding: {error}"))
            })?;
        row_to_stored_credential_binding(&row)
    }

    pub(in crate::session_control) async fn list_credential_bindings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredCredentialBinding>, SessionStoreError> {
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    provider,
                    external_ref,
                    namespace,
                    allowed_origins,
                    injection_mode,
                    totp,
                    labels,
                    created_at,
                    updated_at
                FROM control_credential_bindings
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list credential bindings: {error}"))
            })?;
        rows.iter().map(row_to_stored_credential_binding).collect()
    }

    pub(in crate::session_control) async fn get_credential_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredCredentialBinding>, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    provider,
                    external_ref,
                    namespace,
                    allowed_origins,
                    injection_mode,
                    totp,
                    labels,
                    created_at,
                    updated_at
                FROM control_credential_bindings
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch credential binding: {error}"))
            })?;
        row.map(|row| row_to_stored_credential_binding(&row))
            .transpose()
    }
}
