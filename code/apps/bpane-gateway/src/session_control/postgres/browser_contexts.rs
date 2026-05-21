use super::*;

const BROWSER_CONTEXT_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    name,
    description,
    labels,
    persistence_mode,
    state,
    created_at,
    updated_at,
    last_used_at,
    deleted_at
"#;

pub(super) struct BrowserContextRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn browser_context_repository(&self) -> BrowserContextRepository<'_> {
        BrowserContextRepository { store: self }
    }

    pub(in crate::session_control) async fn create_browser_context(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistBrowserContextRequest,
    ) -> Result<StoredBrowserContext, SessionStoreError> {
        self.browser_context_repository()
            .create_browser_context(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_browser_contexts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredBrowserContext>, SessionStoreError> {
        self.browser_context_repository()
            .list_browser_contexts_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_browser_context_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        self.browser_context_repository()
            .get_browser_context_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn mark_browser_context_used_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        self.browser_context_repository()
            .mark_browser_context_used_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn delete_browser_context_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        self.browser_context_repository()
            .delete_browser_context_for_owner(principal, id)
            .await
    }
}

impl BrowserContextRepository<'_> {
    pub(in crate::session_control) async fn create_browser_context(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistBrowserContextRequest,
    ) -> Result<StoredBrowserContext, SessionStoreError> {
        let now = Utc::now();
        let query = format!(
            r#"
            INSERT INTO control_browser_contexts (
                id,
                owner_subject,
                owner_issuer,
                name,
                description,
                labels,
                persistence_mode,
                state,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, 'ready', $8, $8)
            RETURNING
                {BROWSER_CONTEXT_COLUMNS}
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
                    &request.persistence_mode.as_str(),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "browser context {} already exists",
                        request.name
                    ));
                }
                SessionStoreError::Backend(format!("failed to create browser context: {error}"))
            })?;
        row_to_stored_browser_context(&row)
    }

    pub(in crate::session_control) async fn list_browser_contexts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredBrowserContext>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {BROWSER_CONTEXT_COLUMNS}
            FROM control_browser_contexts
            WHERE owner_subject = $1 AND owner_issuer = $2
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
                SessionStoreError::Backend(format!("failed to list browser contexts: {error}"))
            })?;
        rows.iter().map(row_to_stored_browser_context).collect()
    }

    pub(in crate::session_control) async fn get_browser_context_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {BROWSER_CONTEXT_COLUMNS}
            FROM control_browser_contexts
            WHERE id = $1 AND owner_subject = $2 AND owner_issuer = $3
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
                SessionStoreError::Backend(format!("failed to fetch browser context: {error}"))
            })?;
        row.as_ref().map(row_to_stored_browser_context).transpose()
    }

    pub(in crate::session_control) async fn mark_browser_context_used_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        let query = format!(
            r#"
            UPDATE control_browser_contexts
            SET
                last_used_at = NOW(),
                updated_at = NOW()
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {BROWSER_CONTEXT_COLUMNS}
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
                SessionStoreError::Backend(format!("failed to mark browser context used: {error}"))
            })?;
        row.as_ref().map(row_to_stored_browser_context).transpose()
    }

    pub(in crate::session_control) async fn delete_browser_context_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        let query = format!(
            r#"
            UPDATE control_browser_contexts
            SET
                state = 'deleted',
                deleted_at = COALESCE(deleted_at, NOW()),
                updated_at = NOW()
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {BROWSER_CONTEXT_COLUMNS}
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
                SessionStoreError::Backend(format!("failed to delete browser context: {error}"))
            })?;
        row.as_ref().map(row_to_stored_browser_context).transpose()
    }
}
