use super::*;

impl SessionRepository<'_> {
    pub(in crate::session_control) async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SESSION_COLUMNS}
            FROM control_sessions
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
                SessionStoreError::Backend(format!("failed to list sessions: {error}"))
            })?;

        rows.iter().map(row_to_stored_session).collect()
    }

    pub(in crate::session_control) async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SESSION_COLUMNS}
            FROM control_sessions
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
                SessionStoreError::Backend(format!("failed to load session: {error}"))
            })?;

        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(in crate::session_control) async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SESSION_COLUMNS}
            FROM control_sessions
            WHERE id = $1
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session by id: {error}"))
            })?;

        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(in crate::session_control) async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SESSION_COLUMNS}
            FROM control_sessions
            WHERE id = $1
              AND (
                (owner_subject = $2 AND owner_issuer = $3)
                OR (
                    automation_owner_client_id IS NOT NULL
                    AND automation_owner_issuer = $3
                    AND automation_owner_client_id = $4
                )
              )
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
                    &principal.client_id,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session for principal: {error}"))
            })?;

        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(in crate::session_control) async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {SESSION_COLUMNS}
            FROM control_sessions
            WHERE runtime_binding = $1
              AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
            ORDER BY updated_at DESC
            LIMIT 1
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&self.store.config.runtime_binding])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load runtime candidate session: {error}"
                ))
            })?;

        row.as_ref().map(row_to_stored_session).transpose()
    }
}
