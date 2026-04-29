use super::*;

mod delegation;
mod lifecycle;
mod queries;

const SESSION_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    owner_display_name,
    automation_owner_client_id,
    automation_owner_issuer,
    automation_owner_display_name,
    state,
    template_id,
    owner_mode,
    viewport_width,
    viewport_height,
    idle_timeout_sec,
    labels,
    integration_context,
    extensions,
    recording,
    created_at,
    updated_at,
    stopped_at
"#;

pub(super) struct SessionRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn session_repository(&self) -> SessionRepository<'_> {
        SessionRepository { store: self }
    }

    pub(in crate::session_control) async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        self.session_repository()
            .create_session(principal, request, owner_mode)
            .await
    }

    pub(in crate::session_control) async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        self.session_repository()
            .list_sessions_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .get_session_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository().get_session_by_id(id).await
    }

    pub(in crate::session_control) async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .get_session_for_principal(principal, id)
            .await
    }

    pub(in crate::session_control) async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .get_runtime_candidate_session()
            .await
    }

    pub(in crate::session_control) async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .stop_session_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn mark_session_state(
        &self,
        id: Uuid,
        state: SessionLifecycleState,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .mark_session_state(id, state)
            .await
    }

    pub(in crate::session_control) async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository().stop_session_if_idle(id).await
    }

    pub(in crate::session_control) async fn prepare_session_for_connect(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .prepare_session_for_connect(id)
            .await
    }

    pub(in crate::session_control) async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .set_automation_delegate_for_owner(principal, id, request)
            .await
    }

    pub(in crate::session_control) async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .clear_automation_delegate_for_owner(principal, id)
            .await
    }
}

impl SessionRepository<'_> {
    async fn count_active_runtime_candidates_in_transaction(
        &self,
        transaction: &Transaction<'_>,
    ) -> Result<i64, SessionStoreError> {
        let existing = transaction
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                "#,
                &[&self.store.config.runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to check active sessions: {error}"))
            })?;

        Ok(existing
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0))
    }
}
