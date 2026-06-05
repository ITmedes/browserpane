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
    project_id,
    admission,
    template_id,
    browser_context_mode,
    browser_context_id,
    network_identity,
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
    queued_at,
    runtime_started_at,
    runtime_usage_ms,
    egress_rx_bytes,
    egress_tx_bytes,
    runtime_released_at,
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

    pub(in crate::session_control) async fn cancel_queued_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .cancel_queued_session_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn release_session_runtime_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        self.session_repository()
            .release_session_runtime_for_owner(principal, id)
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

    async fn load_project_for_owner_in_transaction(
        &self,
        transaction: &Transaction<'_>,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<Option<StoredProject>, SessionStoreError> {
        let query = r#"
            SELECT
                id,
                owner_subject,
                owner_issuer,
                name,
                description,
                labels,
                quotas,
                policy,
                state,
                created_at,
                updated_at
            FROM control_projects
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            FOR UPDATE
        "#;
        let row = transaction
            .query_opt(query, &[&project_id, &principal.subject, &principal.issuer])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load project for admission: {error}"))
            })?;
        row.as_ref().map(row_to_stored_project).transpose()
    }

    async fn count_active_sessions_for_project_in_transaction(
        &self,
        transaction: &Transaction<'_>,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        let row = transaction
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                  AND project_id = $3
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                "#,
                &[&principal.subject, &principal.issuer, &project_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to count project active sessions for admission: {error}"
                ))
            })?;
        let count = row
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0);
        u32::try_from(count).map_err(|error| {
            SessionStoreError::Backend(format!(
                "active project session count exceeded u32 range: {error}"
            ))
        })
    }

    async fn count_session_creations_for_project_in_transaction(
        &self,
        transaction: &Transaction<'_>,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        let row = transaction
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                  AND project_id = $3
                "#,
                &[&principal.subject, &principal.issuer, &project_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to count project session creations for admission: {error}"
                ))
            })?;
        let count = row
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0);
        u32::try_from(count).map_err(|error| {
            SessionStoreError::Backend(format!(
                "project session creation count exceeded u32 range: {error}"
            ))
        })
    }

    async fn count_session_creations_for_project_since_in_transaction(
        &self,
        transaction: &Transaction<'_>,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
        window_started_at: DateTime<Utc>,
    ) -> Result<u32, SessionStoreError> {
        let row = transaction
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                  AND project_id = $3
                  AND created_at >= $4
                "#,
                &[
                    &principal.subject,
                    &principal.issuer,
                    &project_id,
                    &window_started_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to count project session creations in rate window for admission: {error}"
                ))
            })?;
        let count = row
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0);
        u32::try_from(count).map_err(|error| {
            SessionStoreError::Backend(format!(
                "project session creation rate-window count exceeded u32 range: {error}"
            ))
        })
    }

    async fn sum_runtime_usage_ms_for_project_in_transaction(
        &self,
        transaction: &Transaction<'_>,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
        observed_at: DateTime<Utc>,
    ) -> Result<u64, SessionStoreError> {
        let row = transaction
            .query_one(
                r#"
                SELECT COALESCE(SUM(
                    runtime_usage_ms
                    + CASE
                        WHEN runtime_started_at IS NOT NULL
                         AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                            THEN GREATEST(
                                0,
                                FLOOR(EXTRACT(EPOCH FROM ($4::timestamptz - runtime_started_at)) * 1000)
                            )::BIGINT
                        ELSE 0
                    END
                ), 0)::BIGINT AS runtime_usage_ms
                FROM control_sessions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                  AND project_id = $3
                "#,
                &[
                    &principal.subject,
                    &principal.issuer,
                    &project_id,
                    &observed_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to sum project runtime usage milliseconds for admission: {error}"
                ))
            })?;
        let runtime_usage_ms = row.get::<_, i64>("runtime_usage_ms");
        u64::try_from(runtime_usage_ms).map_err(|error| {
            SessionStoreError::Backend(format!(
                "project runtime usage milliseconds exceeded u64 range: {error}"
            ))
        })
    }
}
