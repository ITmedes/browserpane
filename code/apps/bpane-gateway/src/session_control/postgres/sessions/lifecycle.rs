use super::*;

impl SessionRepository<'_> {
    pub(in crate::session_control) async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let active_runtime_candidates = self
            .count_active_runtime_candidates_in_transaction(&transaction)
            .await?;
        if active_runtime_candidates >= self.store.config.max_runtime_candidates as i64 {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.store.config.max_runtime_candidates,
            });
        }

        let viewport = request.viewport.unwrap_or_default();
        let now = Utc::now();
        let admission = if let Some(project_id) = request.project_id {
            let project = self
                .load_project_for_owner_in_transaction(&transaction, principal, project_id)
                .await?
                .ok_or_else(|| {
                    SessionStoreError::NotFound(format!("project {project_id} not found"))
                })?;
            let active_project_sessions = self
                .count_active_sessions_for_project_in_transaction(
                    &transaction,
                    principal,
                    project_id,
                )
                .await?;
            if project.state == ProjectState::Archived {
                let decision = ProjectAdmissionDecision::rejected(
                    project_id,
                    ProjectAdmissionReasonCode::ProjectArchived,
                    format!("project {project_id} is archived"),
                    active_project_sessions,
                    project.quotas.max_active_sessions,
                    now,
                );
                return Err(SessionStoreError::Conflict(format!(
                    "project admission rejected: {}: {}",
                    decision.reason_code.as_str(),
                    decision.message
                )));
            }
            if let Some(max_active_sessions) = project.quotas.max_active_sessions {
                if active_project_sessions >= max_active_sessions {
                    let decision = ProjectAdmissionDecision::rejected(
                        project_id,
                        ProjectAdmissionReasonCode::ActiveSessionQuotaExceeded,
                        format!(
                            "project {project_id} active session quota is exhausted ({active_project_sessions}/{max_active_sessions})"
                        ),
                        active_project_sessions,
                        Some(max_active_sessions),
                        now,
                    );
                    return Err(SessionStoreError::Conflict(format!(
                        "project admission rejected: {}: {}",
                        decision.reason_code.as_str(),
                        decision.message
                    )));
                }
            }
            ProjectAdmissionDecision::project_quota_available(
                project_id,
                active_project_sessions.saturating_add(1),
                project.quotas.max_active_sessions,
                now,
            )
        } else {
            ProjectAdmissionDecision::owner_scope_unbounded(now)
        };
        let admission_value = serde_json::to_value(&admission).map_err(|error| {
            SessionStoreError::Backend(format!("failed to encode session admission: {error}"))
        })?;
        let labels_value = json_labels(&request.labels);
        let extensions_value = json_applied_extensions(&request.extensions)?;
        let recording_value = json_recording_policy(&request.recording)?;
        let network_identity_value = serde_json::to_value(
            request.network_identity.clone().unwrap_or_default(),
        )
        .map_err(|error| {
            SessionStoreError::Backend(format!(
                "failed to encode session network identity: {error}"
            ))
        })?;
        let browser_context = request.browser_context.clone().unwrap_or_default();
        let session_id = Uuid::now_v7();
        let insert_query = format!(
            r#"
            INSERT INTO control_sessions (
                id,
                owner_subject,
                owner_issuer,
                owner_display_name,
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
                runtime_binding,
                created_at,
                updated_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7::jsonb, $8, $9, $10, $11::jsonb, $12, $13, $14, $15,
                $16::jsonb, $17::jsonb, $18::jsonb, $19::jsonb, $20, $21, $21
            )
            RETURNING
                {SESSION_COLUMNS}
            "#
        );
        let row = transaction
            .query_one(
                &insert_query,
                &[
                    &session_id,
                    &principal.subject,
                    &principal.issuer,
                    &principal.display_name,
                    &SessionLifecycleState::Ready.as_str(),
                    &request.project_id,
                    &admission_value,
                    &request.template_id,
                    &browser_context.mode.as_str(),
                    &browser_context.context_id,
                    &network_identity_value,
                    &owner_mode.as_str(),
                    &(viewport.width as i32),
                    &(viewport.height as i32),
                    &request.idle_timeout_sec.map(|value| value as i32),
                    &labels_value,
                    &request.integration_context,
                    &extensions_value,
                    &recording_value,
                    &self.store.config.runtime_binding,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert session: {error}"))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_session(&row)
    }

    pub(in crate::session_control) async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let update_query = format!(
            r#"
            UPDATE control_sessions
            SET
                state = 'stopped',
                updated_at = NOW(),
                stopped_at = COALESCE(stopped_at, NOW())
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {SESSION_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&update_query, &[&id, &principal.subject, &principal.issuer])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop session: {error}"))
            })?;

        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(in crate::session_control) async fn release_session_runtime_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let current = self
            .get_session_for_owner(principal, id)
            .await?
            .ok_or_else(|| SessionStoreError::NotFound(format!("session {id} not found")))?;
        if current.state == SessionLifecycleState::Stopped {
            return Err(SessionStoreError::Conflict(format!(
                "session {id} is stopped; create a new session instead of releasing it"
            )));
        }
        if !current.state.is_runtime_candidate() && current.state != SessionLifecycleState::Released
        {
            return Err(SessionStoreError::Conflict(format!(
                "session {id} cannot release a runtime from state {}",
                current.state.as_str()
            )));
        }

        let update_query = format!(
            r#"
            UPDATE control_sessions
            SET
                state = 'released',
                updated_at = NOW(),
                runtime_released_at = NOW(),
                stopped_at = NULL
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {SESSION_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&update_query, &[&id, &principal.subject, &principal.issuer])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to release session runtime: {error}"))
            })?;

        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(in crate::session_control) async fn mark_session_state(
        &self,
        id: Uuid,
        state: SessionLifecycleState,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let update_query = format!(
            r#"
            UPDATE control_sessions
            SET
                state = $2,
                updated_at = CASE WHEN state = $2 THEN updated_at ELSE NOW() END
            WHERE id = $1
              AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
            RETURNING
                {SESSION_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&update_query, &[&id, &state.as_str()])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update session state: {error}"))
            })?;

        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(in crate::session_control) async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let update_query = format!(
            r#"
            UPDATE control_sessions
            SET
                state = 'stopped',
                updated_at = NOW(),
                stopped_at = COALESCE(stopped_at, NOW())
            WHERE id = $1
              AND state IN ('ready', 'idle')
            RETURNING
                {SESSION_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&update_query, &[&id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop idle session: {error}"))
            })?;

        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(in crate::session_control) async fn prepare_session_for_connect(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let load_query = format!(
            r#"
            SELECT
                {SESSION_COLUMNS}
            FROM control_sessions
            WHERE id = $1
            FOR UPDATE
            "#
        );
        let current_row = transaction
            .query_opt(&load_query, &[&id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock session for connect prep: {error}"
                ))
            })?;
        let Some(current_row) = current_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };

        let current = row_to_stored_session(&current_row)?;
        if current.state != SessionLifecycleState::Released
            && current.state != SessionLifecycleState::Stopped
        {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(Some(current));
        }
        let active_runtime_candidates = self
            .count_active_runtime_candidates_in_transaction(&transaction)
            .await?;
        if active_runtime_candidates >= self.store.config.max_runtime_candidates as i64 {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.store.config.max_runtime_candidates,
            });
        }

        let update_query = format!(
            r#"
            UPDATE control_sessions
            SET
                state = 'ready',
                updated_at = NOW(),
                runtime_released_at = COALESCE(stopped_at, runtime_released_at, NOW()),
                stopped_at = NULL
            WHERE id = $1
            RETURNING
                {SESSION_COLUMNS}
            "#
        );
        let row = transaction
            .query_one(&update_query, &[&id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to prepare stopped session for connect: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_session(&row).map(Some)
    }
}
