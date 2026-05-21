use super::*;

const SESSION_TEMPLATE_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    name,
    description,
    labels,
    defaults,
    version,
    created_at,
    updated_at
"#;

pub(super) struct SessionTemplateRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn session_template_repository(&self) -> SessionTemplateRepository<'_> {
        SessionTemplateRepository { store: self }
    }

    pub(in crate::session_control) async fn create_session_template(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistSessionTemplateRequest,
    ) -> Result<StoredSessionTemplate, SessionStoreError> {
        self.session_template_repository()
            .create_session_template(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_session_templates_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSessionTemplate>, SessionStoreError> {
        self.session_template_repository()
            .list_session_templates_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_session_template_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSessionTemplate>, SessionStoreError> {
        self.session_template_repository()
            .get_session_template_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn update_session_template_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistSessionTemplateRequest,
    ) -> Result<Option<StoredSessionTemplate>, SessionStoreError> {
        self.session_template_repository()
            .update_session_template_for_owner(principal, id, request)
            .await
    }
}

impl SessionTemplateRepository<'_> {
    async fn create_session_template(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistSessionTemplateRequest,
    ) -> Result<StoredSessionTemplate, SessionStoreError> {
        let now = Utc::now();
        let defaults_value = serde_json::to_value(&request.defaults).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "session template defaults must be serializable: {error}"
            ))
        })?;
        let query = format!(
            r#"
            INSERT INTO control_session_templates (
                id,
                owner_subject,
                owner_issuer,
                name,
                description,
                labels,
                defaults,
                version,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::jsonb, 1, $8, $8)
            RETURNING
                {SESSION_TEMPLATE_COLUMNS}
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
                    &defaults_value,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "session template {} already exists",
                        request.name
                    ));
                }
                SessionStoreError::Backend(format!("failed to create session template: {error}"))
            })?;
        row_to_stored_session_template(&row)
    }

    async fn list_session_templates_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSessionTemplate>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SESSION_TEMPLATE_COLUMNS}
            FROM control_session_templates
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
                SessionStoreError::Backend(format!("failed to list session templates: {error}"))
            })?;
        rows.iter().map(row_to_stored_session_template).collect()
    }

    async fn get_session_template_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSessionTemplate>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SESSION_TEMPLATE_COLUMNS}
            FROM control_session_templates
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
                SessionStoreError::Backend(format!("failed to fetch session template: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session_template).transpose()
    }

    async fn update_session_template_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistSessionTemplateRequest,
    ) -> Result<Option<StoredSessionTemplate>, SessionStoreError> {
        let defaults_value = serde_json::to_value(&request.defaults).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "session template defaults must be serializable: {error}"
            ))
        })?;
        let query = format!(
            r#"
            UPDATE control_session_templates
            SET
                name = $4,
                description = $5,
                labels = $6::jsonb,
                defaults = $7::jsonb,
                version = version + 1,
                updated_at = NOW()
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {SESSION_TEMPLATE_COLUMNS}
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
                    &defaults_value,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "session template {} already exists",
                        request.name
                    ));
                }
                SessionStoreError::Backend(format!("failed to update session template: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session_template).transpose()
    }
}
