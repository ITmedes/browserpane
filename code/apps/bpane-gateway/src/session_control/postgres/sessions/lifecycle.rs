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
        let labels_value = json_labels(&request.labels);
        let extensions_value = json_applied_extensions(&request.extensions)?;
        let recording_value = json_recording_policy(&request.recording)?;
        let session_id = Uuid::now_v7();
        let insert_query = format!(
            r#"
            INSERT INTO control_sessions (
                id,
                owner_subject,
                owner_issuer,
                owner_display_name,
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
                runtime_binding,
                created_at,
                updated_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::jsonb, $12::jsonb,
                $13::jsonb, $14::jsonb, $15, $16, $16
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
                    &request.template_id,
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
                updated_at = NOW()
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
        if current.state != SessionLifecycleState::Stopped {
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
