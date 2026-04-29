use super::*;

mod automation_tasks;
mod credential_bindings;
mod db;
mod extensions;
mod file_workspaces;
mod recordings;
mod runtime_assignments;
mod workflow_definitions;

use db::*;

pub(super) struct PostgresSessionStore {
    pub(super) db: PostgresDb,
    config: SessionStoreConfig,
}

impl PostgresSessionStore {
    pub(super) async fn connect(
        database_url: &str,
        config: SessionStoreConfig,
    ) -> Result<Self, SessionStoreError> {
        let db = PostgresDb::connect(database_url).await?;
        Ok(Self { db, config })
    }

    pub(super) async fn enqueue_workflow_event_deliveries(
        transaction: &Transaction<'_>,
        run: &StoredWorkflowRun,
        event: &StoredWorkflowRunEvent,
    ) -> Result<(), SessionStoreError> {
        let rows = transaction
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                FROM control_workflow_event_subscriptions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                "#,
                &[&run.owner_subject, &run.owner_issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow event subscriptions for delivery enqueue: {error}"
                ))
            })?;

        for row in rows {
            let subscription = row_to_stored_workflow_event_subscription(&row)?;
            if !workflow_event_type_matches(&subscription.event_types, &event.event_type) {
                continue;
            }
            let delivery_id = Uuid::now_v7();
            let payload =
                build_workflow_event_delivery_payload(subscription.id, delivery_id, run, event);
            transaction
                .execute(
                    r#"
                    INSERT INTO control_workflow_event_deliveries (
                        id,
                        subscription_id,
                        run_id,
                        event_id,
                        event_type,
                        target_url,
                        signing_secret,
                        payload,
                        state,
                        attempt_count,
                        next_attempt_at,
                        last_attempt_at,
                        delivered_at,
                        last_response_status,
                        last_error,
                        created_at,
                        updated_at
                    )
                    VALUES (
                        $1, $2, $3, $4, $5, $6, $7, $8::jsonb, 'pending',
                        0, $9, NULL, NULL, NULL, NULL, $9, $9
                    )
                    "#,
                    &[
                        &delivery_id,
                        &subscription.id,
                        &run.id,
                        &event.id,
                        &event.event_type,
                        &subscription.target_url,
                        &subscription.signing_secret,
                        &payload,
                        &event.created_at,
                    ],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to insert workflow event delivery: {error}"
                    ))
                })?;
        }
        Ok(())
    }

    pub(super) async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let existing = transaction
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                "#,
                &[&self.config.runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to check active sessions: {error}"))
            })?;
        let active_runtime_candidates = existing
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0);
        if active_runtime_candidates >= self.config.max_runtime_candidates as i64 {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let viewport = request.viewport.unwrap_or_default();
        let now = Utc::now();
        let labels_value = json_labels(&request.labels);
        let extensions_value = json_applied_extensions(&request.extensions)?;
        let recording_value = json_recording_policy(&request.recording)?;
        let session_id = Uuid::now_v7();
        let row = transaction
            .query_one(
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
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11::jsonb, $12::jsonb, $13::jsonb, $14::jsonb, $15, $16, $16)
                RETURNING
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
                "#,
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
                    &self.config.runtime_binding,
                    &now,
                ],
            )
            .await
            .map_err(|error| SessionStoreError::Backend(format!("failed to insert session: {error}")))?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_session(&row)
    }

    pub(super) async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
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
                FROM control_sessions
                WHERE owner_subject = $1 AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list sessions: {error}"))
            })?;

        rows.iter().map(row_to_stored_session).collect()
    }

    pub(super) async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
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
                FROM control_sessions
                WHERE id = $1 AND owner_subject = $2 AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(super) async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
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
                FROM control_sessions
                WHERE id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load session by id: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(super) async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
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
                "#,
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

    pub(super) async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
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
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                ORDER BY updated_at DESC
                LIMIT 1
                "#,
                &[&self.config.runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load runtime candidate session: {error}"
                ))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(super) async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
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
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop session: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(super) async fn mark_session_state(
        &self,
        id: Uuid,
        state: SessionLifecycleState,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = $2,
                    updated_at = NOW()
                WHERE id = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                RETURNING
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
                "#,
                &[&id, &state.as_str()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update session state: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(super) async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    state = 'stopped',
                    updated_at = NOW(),
                    stopped_at = COALESCE(stopped_at, NOW())
                WHERE id = $1
                  AND state IN ('ready', 'idle')
                RETURNING
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
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to stop idle session: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(super) async fn prepare_session_for_connect(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let current_row = transaction
            .query_opt(
                r#"
                SELECT
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
                FROM control_sessions
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
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

        let existing = transaction
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                "#,
                &[&self.config.runtime_binding],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to check active sessions: {error}"))
            })?;
        let active_runtime_candidates = existing
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0);
        if active_runtime_candidates >= self.config.max_runtime_candidates as i64 {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let row = transaction
            .query_one(
                r#"
                UPDATE control_sessions
                SET
                    state = 'ready',
                    updated_at = NOW(),
                    stopped_at = NULL
                WHERE id = $1
                RETURNING
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
                "#,
                &[&id],
            )
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

    pub(super) async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let issuer = request.issuer.unwrap_or_else(|| principal.issuer.clone());
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    automation_owner_client_id = $4,
                    automation_owner_issuer = $5,
                    automation_owner_display_name = $6,
                    updated_at = NOW()
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
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
                "#,
                &[
                    &id,
                    &principal.subject,
                    &principal.issuer,
                    &request.client_id,
                    &issuer,
                    &request.display_name,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to set automation delegate: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(super) async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_sessions
                SET
                    automation_owner_client_id = NULL,
                    automation_owner_issuer = NULL,
                    automation_owner_display_name = NULL,
                    updated_at = NOW()
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
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
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to clear automation delegate: {error}"))
            })?;
        row.as_ref().map(row_to_stored_session).transpose()
    }

    pub(super) async fn create_workflow_run(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowRunRequest,
    ) -> Result<CreateWorkflowRunResult, SessionStoreError> {
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let workflow_row = transaction
            .query_opt(
                r#"
                SELECT id
                FROM control_workflow_definitions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &request.workflow_definition_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate workflow definition for run: {error}"
                ))
            })?;
        if workflow_row.is_none() {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition {} not found",
                request.workflow_definition_id
            )));
        }

        let version_row = transaction
            .query_opt(
                r#"
                SELECT id, workflow_definition_id, version
                FROM control_workflow_definition_versions
                WHERE id = $1
                "#,
                &[&request.workflow_definition_version_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate workflow definition version for run: {error}"
                ))
            })?;
        let Some(version_row) = version_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "workflow definition version {} not found",
                request.workflow_definition_version_id
            )));
        };
        let version_workflow_id: Uuid = version_row.get("workflow_definition_id");
        let version_name: String = version_row.get("version");
        if version_workflow_id != request.workflow_definition_id
            || version_name != request.workflow_version
        {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::InvalidRequest(
                "workflow run version must belong to the requested workflow definition".to_string(),
            ));
        }

        let task_row = transaction
            .query_opt(
                r#"
                SELECT id, session_id
                FROM control_automation_tasks
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &request.automation_task_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate automation task for workflow run: {error}"
                ))
            })?;
        let Some(task_row) = task_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "automation task {} not found",
                request.automation_task_id
            )));
        };
        let task_session_id: Uuid = task_row.get("session_id");
        if task_session_id != request.session_id {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::InvalidRequest(
                "workflow run session_id must match the bound automation task session".to_string(),
            ));
        }

        let now = Utc::now();
        let source_snapshot = json_workflow_run_source_snapshot(request.source_snapshot.as_ref())?;
        let extensions = json_applied_extensions(&request.extensions)?;
        let credential_bindings =
            json_workflow_run_credential_bindings(&request.credential_bindings)?;
        let workspace_inputs = json_workflow_run_workspace_inputs(&request.workspace_inputs)?;
        let produced_files = json_workflow_run_produced_files(&Vec::new())?;
        if let Some(client_request_id) = request.client_request_id.as_deref() {
            let existing_row = transaction
                .query_opt(
                    r#"
                    SELECT
                        id,
                        owner_subject,
                        owner_issuer,
                        workflow_definition_id,
                        workflow_definition_version_id,
                        workflow_version,
                        session_id,
                        automation_task_id,
                        state,
                        source_system,
                        source_reference,
                        client_request_id,
                        create_request_fingerprint,
                        source_snapshot,
                        extensions,
                        credential_bindings,
                        workspace_inputs,
                        produced_files,
                        input,
                        output,
                        error,
                        artifact_refs,
                        labels,
                        started_at,
                        completed_at,
                        created_at,
                        updated_at
                    FROM control_workflow_runs
                    WHERE owner_subject = $1
                      AND owner_issuer = $2
                      AND client_request_id = $3
                    "#,
                    &[&principal.subject, &principal.issuer, &client_request_id],
                )
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to check existing workflow run by client_request_id: {error}"
                    ))
                })?;
            if let Some(existing_row) = existing_row {
                let existing_run = row_to_stored_workflow_run(&existing_row)?;
                transaction.commit().await.map_err(|error| {
                    SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
                })?;
                if existing_run.create_request_fingerprint == request.create_request_fingerprint {
                    return Ok(CreateWorkflowRunResult {
                        run: existing_run,
                        created: false,
                    });
                }
                return Err(SessionStoreError::Conflict(format!(
                    "workflow run client_request_id {} is already bound to a different request",
                    client_request_id
                )));
            }
        }
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_workflow_runs (
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                )
                VALUES (
                    $1, $2, $3, $4, $5, $6, $7, $8, $9,
                    $10, $11, $12, $13,
                    $14::jsonb, $15::jsonb, $16::jsonb, $17::jsonb, $18::jsonb, $19::jsonb, NULL, NULL, $20::jsonb, $21::jsonb, NULL, NULL, $22, $22
                )
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.workflow_definition_id,
                    &request.workflow_definition_version_id,
                    &request.workflow_version,
                    &request.session_id,
                    &request.automation_task_id,
                    &WorkflowRunState::Pending.as_str(),
                    &request.source_system,
                    &request.source_reference,
                    &request.client_request_id,
                    &request.create_request_fingerprint,
                    &source_snapshot,
                    &extensions,
                    &credential_bindings,
                    &workspace_inputs,
                    &produced_files,
                    &request.input,
                    &json_string_array(&Vec::new()),
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert workflow run: {error}"))
            })?;
        let run = row_to_stored_workflow_run(&row)?;
        let run_id = run.id;
        let event_id = Uuid::now_v7();
        let event = StoredWorkflowRunEvent {
            id: event_id,
            run_id,
            event_type: "workflow_run.created".to_string(),
            message: "workflow run created".to_string(),
            data: Some(serde_json::json!({
                "workflow_definition_id": request.workflow_definition_id,
                "workflow_definition_version_id": request.workflow_definition_version_id,
                "workflow_version": request.workflow_version,
                "session_id": request.session_id,
                "automation_task_id": request.automation_task_id,
                "source_system": request.source_system,
                "source_reference": request.source_reference,
                "client_request_id": request.client_request_id,
            })),
            created_at: now,
        };

        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_run_events (
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &event_id,
                    &run_id,
                    &"workflow_run.created",
                    &"workflow run created",
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to insert workflow run event: {error}"))
            })?;
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(CreateWorkflowRunResult { run, created: true })
    }

    pub(super) async fn get_workflow_run_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    run.id,
                    run.owner_subject,
                    run.owner_issuer,
                    run.workflow_definition_id,
                    run.workflow_definition_version_id,
                    run.workflow_version,
                    run.session_id,
                    run.automation_task_id,
                    run.state,
                    run.source_system,
                    run.source_reference,
                    run.client_request_id,
                    run.create_request_fingerprint,
                    run.source_snapshot,
                    run.extensions,
                    run.credential_bindings,
                    run.workspace_inputs,
                    run.produced_files,
                    run.input,
                    run.output,
                    run.error,
                    run.artifact_refs,
                    run.labels,
                    run.started_at,
                    run.completed_at,
                    run.created_at,
                    run.updated_at
                FROM control_workflow_runs run
                JOIN control_workflow_definitions workflow
                  ON workflow.id = run.workflow_definition_id
                WHERE run.id = $1
                  AND workflow.owner_subject = $2
                  AND workflow.owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load workflow run: {error}"))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }

    pub(super) async fn get_workflow_run_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE id = $1
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to load workflow run by id: {error}"))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }

    pub(super) async fn list_dispatchable_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE state IN ('pending', 'queued')
                ORDER BY created_at ASC, id ASC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list dispatchable workflow runs: {error}"
                ))
            })?;
        rows.into_iter()
            .map(|row| row_to_stored_workflow_run(&row))
            .collect()
    }

    pub(super) async fn find_workflow_run_by_client_request_id_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        client_request_id: &str,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                  AND client_request_id = $3
                "#,
                &[&principal.subject, &principal.issuer, &client_request_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to find workflow run by client_request_id: {error}"
                ))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }

    pub(super) async fn create_workflow_event_subscription(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEventSubscriptionRequest,
    ) -> Result<StoredWorkflowEventSubscription, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .db
            .client()
            .await?
            .query_one(
                r#"
                INSERT INTO control_workflow_event_subscriptions (
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8, $8)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.target_url,
                    &json_string_array(&request.event_types),
                    &request.signing_secret,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow event subscription: {error}"
                ))
            })?;
        row_to_stored_workflow_event_subscription(&row)
    }

    pub(super) async fn list_workflow_event_subscriptions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredWorkflowEventSubscription>, SessionStoreError> {
        let rows = self
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
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                FROM control_workflow_event_subscriptions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC, id DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow event subscriptions: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_workflow_event_subscription)
            .collect()
    }

    pub(super) async fn get_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        let row = self
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
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                FROM control_workflow_event_subscriptions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load workflow event subscription: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_event_subscription)
            .transpose()
    }

    pub(super) async fn delete_workflow_event_subscription_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowEventSubscription>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                DELETE FROM control_workflow_event_subscriptions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    target_url,
                    event_types,
                    signing_secret,
                    created_at,
                    updated_at
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete workflow event subscription: {error}"
                ))
            })?;
        row.as_ref()
            .map(row_to_stored_workflow_event_subscription)
            .transpose()
    }

    pub(super) async fn list_workflow_event_deliveries_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        if self
            .get_workflow_event_subscription_for_owner(principal, subscription_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    subscription_id,
                    run_id,
                    event_id,
                    event_type,
                    target_url,
                    signing_secret,
                    payload,
                    state,
                    attempt_count,
                    next_attempt_at,
                    last_attempt_at,
                    delivered_at,
                    last_response_status,
                    last_error,
                    created_at,
                    updated_at
                FROM control_workflow_event_deliveries
                WHERE subscription_id = $1
                ORDER BY created_at ASC, event_id ASC, id ASC
                "#,
                &[&subscription_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow event deliveries: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_workflow_event_delivery)
            .collect()
    }

    pub(super) async fn list_workflow_event_delivery_attempts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        subscription_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEventDeliveryAttempt>, SessionStoreError> {
        if self
            .get_workflow_event_subscription_for_owner(principal, subscription_id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    attempt.id,
                    attempt.delivery_id,
                    attempt.attempt_number,
                    attempt.response_status,
                    attempt.error,
                    attempt.created_at
                FROM control_workflow_event_delivery_attempts attempt
                JOIN control_workflow_event_deliveries delivery
                  ON delivery.id = attempt.delivery_id
                WHERE delivery.subscription_id = $1
                ORDER BY attempt.created_at ASC, attempt.id ASC
                "#,
                &[&subscription_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow event delivery attempts: {error}"
                ))
            })?;
        rows.iter()
            .map(row_to_stored_workflow_event_delivery_attempt)
            .collect()
    }

    pub(super) async fn requeue_inflight_workflow_event_deliveries(
        &self,
    ) -> Result<(), SessionStoreError> {
        self.db
            .client()
            .await?
            .execute(
                r#"
                UPDATE control_workflow_event_deliveries
                SET
                    state = 'pending',
                    next_attempt_at = NOW(),
                    updated_at = NOW()
                WHERE state = 'delivering'
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to requeue inflight workflow event deliveries: {error}"
                ))
            })?;
        Ok(())
    }

    pub(super) async fn claim_due_workflow_event_deliveries(
        &self,
        limit: usize,
        now: DateTime<Utc>,
    ) -> Result<Vec<StoredWorkflowEventDelivery>, SessionStoreError> {
        let limit = i64::try_from(limit).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "workflow event delivery limit is out of range: {error}"
            ))
        })?;
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                WITH claimed AS (
                    SELECT id
                    FROM control_workflow_event_deliveries
                    WHERE state = 'pending'
                      AND (next_attempt_at IS NULL OR next_attempt_at <= $2)
                    ORDER BY created_at ASC, event_id ASC, id ASC
                    FOR UPDATE SKIP LOCKED
                    LIMIT $1
                )
                UPDATE control_workflow_event_deliveries delivery
                SET
                    state = 'delivering',
                    updated_at = $2
                FROM claimed
                WHERE delivery.id = claimed.id
                RETURNING
                    delivery.id,
                    delivery.subscription_id,
                    delivery.run_id,
                    delivery.event_id,
                    delivery.event_type,
                    delivery.target_url,
                    delivery.signing_secret,
                    delivery.payload,
                    delivery.state,
                    delivery.attempt_count,
                    delivery.next_attempt_at,
                    delivery.last_attempt_at,
                    delivery.delivered_at,
                    delivery.last_response_status,
                    delivery.last_error,
                    delivery.created_at,
                    delivery.updated_at
                "#,
                &[&limit, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to claim due workflow event deliveries: {error}"
                ))
            })?;
        let mut deliveries = rows
            .iter()
            .map(row_to_stored_workflow_event_delivery)
            .collect::<Result<Vec<_>, _>>()?;
        deliveries.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.event_id.cmp(&right.event_id))
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(deliveries)
    }

    pub(super) async fn record_workflow_event_delivery_attempt(
        &self,
        delivery_id: Uuid,
        request: RecordWorkflowEventDeliveryAttemptRequest,
    ) -> Result<Option<StoredWorkflowEventDelivery>, SessionStoreError> {
        let response_status = request.response_status.map(i32::from);
        let attempt_number = i32::try_from(request.attempt_number).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "workflow event delivery attempt_number is out of range: {error}"
            ))
        })?;
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let row = transaction
            .query_opt(
                r#"
                UPDATE control_workflow_event_deliveries
                SET
                    state = $2,
                    attempt_count = $3,
                    next_attempt_at = $4,
                    last_attempt_at = $5,
                    delivered_at = $6,
                    last_response_status = $7,
                    last_error = $8,
                    updated_at = $5
                WHERE id = $1
                RETURNING
                    id,
                    subscription_id,
                    run_id,
                    event_id,
                    event_type,
                    target_url,
                    signing_secret,
                    payload,
                    state,
                    attempt_count,
                    next_attempt_at,
                    last_attempt_at,
                    delivered_at,
                    last_response_status,
                    last_error,
                    created_at,
                    updated_at
                "#,
                &[
                    &delivery_id,
                    &request.state.as_str(),
                    &attempt_number,
                    &request.next_attempt_at,
                    &request.attempted_at,
                    &request.delivered_at,
                    &response_status,
                    &request.error,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow event delivery attempt: {error}"
                ))
            })?;
        let Some(row) = row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_event_delivery_attempts (
                    id,
                    delivery_id,
                    attempt_number,
                    response_status,
                    error,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5, $6)
                "#,
                &[
                    &Uuid::now_v7(),
                    &delivery_id,
                    &attempt_number,
                    &response_status,
                    &request.error,
                    &request.attempted_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow event delivery attempt: {error}"
                ))
            })?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(Some(row_to_stored_workflow_event_delivery(&row)?))
    }

    pub(super) async fn list_workflow_run_events_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                FROM control_workflow_run_events
                WHERE run_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow run events: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run_event).collect()
    }

    pub(super) async fn list_workflow_run_events(
        &self,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunEvent>, SessionStoreError> {
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                FROM control_workflow_run_events
                WHERE run_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow run events: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run_event).collect()
    }

    pub(super) async fn list_workflow_run_logs_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Vec<StoredWorkflowRunLog>, SessionStoreError> {
        if self
            .get_workflow_run_for_owner(principal, id)
            .await?
            .is_none()
        {
            return Ok(Vec::new());
        }
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    run_id,
                    stream,
                    message,
                    created_at
                FROM control_workflow_run_logs
                WHERE run_id = $1
                ORDER BY created_at ASC, id ASC
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow run logs: {error}"))
            })?;
        rows.iter().map(row_to_stored_workflow_run_log).collect()
    }

    pub(super) async fn append_workflow_run_event_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        let Some(run) = self.get_workflow_run_for_owner(principal, id).await? else {
            return Ok(None);
        };
        let now = Utc::now();
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let event_id = Uuid::now_v7();
        let event = StoredWorkflowRunEvent {
            id: event_id,
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: now,
        };
        let row = transaction
            .query_opt(
                r#"
                WITH inserted AS (
                    INSERT INTO control_workflow_run_events (
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                    )
                    VALUES ($2, $1, $3, $4, $5::jsonb, $6)
                    RETURNING
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                )
                UPDATE control_workflow_runs
                SET updated_at = $6
                WHERE id = $1
                RETURNING (SELECT id FROM inserted) AS inserted_id
                "#,
                &[
                    &id,
                    &event_id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append workflow run event: {error}"))
            })?;
        let Some(row) = row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let inserted_id: Uuid = row.get("inserted_id");
        if inserted_id != event.id {
            return Err(SessionStoreError::Backend(
                "workflow run event insert returned unexpected id".to_string(),
            ));
        }
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(Some(event))
    }

    pub(super) async fn append_workflow_run_event(
        &self,
        id: Uuid,
        request: PersistWorkflowRunEventRequest,
    ) -> Result<Option<StoredWorkflowRunEvent>, SessionStoreError> {
        let Some(run) = self.get_workflow_run_by_id(id).await? else {
            return Ok(None);
        };
        let now = Utc::now();
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let event_id = Uuid::now_v7();
        let event = StoredWorkflowRunEvent {
            id: event_id,
            run_id: id,
            event_type: request.event_type,
            message: request.message,
            data: request.data,
            created_at: now,
        };
        let row = transaction
            .query_opt(
                r#"
                WITH inserted AS (
                    INSERT INTO control_workflow_run_events (
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                    )
                    VALUES ($2, $1, $3, $4, $5::jsonb, $6)
                    RETURNING
                        id,
                        run_id,
                        event_type,
                        message,
                        data,
                        created_at
                )
                UPDATE control_workflow_runs
                SET updated_at = $6
                WHERE id = $1
                RETURNING (SELECT id FROM inserted) AS inserted_id
                "#,
                &[
                    &id,
                    &event_id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append workflow run event: {error}"))
            })?;
        let Some(row) = row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let inserted_id: Uuid = row.get("inserted_id");
        if inserted_id != event.id {
            return Err(SessionStoreError::Backend(
                "workflow run event insert returned unexpected id".to_string(),
            ));
        }
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(Some(event))
    }

    pub(super) async fn transition_workflow_run(
        &self,
        id: Uuid,
        request: WorkflowRunTransitionRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let run_row = transaction
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock workflow run for transition: {error}"
                ))
            })?;
        let Some(run_row) = run_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current_run = row_to_stored_workflow_run(&run_row)?;

        let task_row = transaction
            .query_one(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&current_run.automation_task_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for workflow run transition: {error}"
                ))
            })?;
        let current_task = row_to_stored_automation_task(&task_row)?;
        let task_state: AutomationTaskState = request.state.into();
        if current_task.state.is_terminal() {
            return Err(SessionStoreError::Conflict(format!(
                "automation task {} is already terminal",
                current_task.id
            )));
        }
        if !current_task.state.can_transition_to(task_state) {
            return Err(SessionStoreError::Conflict(format!(
                "automation task {} cannot transition from {} to {}",
                current_task.id,
                current_task.state.as_str(),
                task_state.as_str()
            )));
        }

        let now = Utc::now();
        let started_at = if matches!(
            task_state,
            AutomationTaskState::Starting
                | AutomationTaskState::Running
                | AutomationTaskState::AwaitingInput
        ) {
            current_task.started_at.or(Some(now))
        } else {
            current_task.started_at
        };
        let completed_at = if task_state.is_terminal() {
            Some(now)
        } else {
            current_task.completed_at
        };
        let artifact_refs = json_string_array(&request.artifact_refs);
        let task_row = transaction
            .query_one(
                r#"
                UPDATE control_automation_tasks
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &current_task.id,
                    &task_state.as_str(),
                    &request.output,
                    &request.error,
                    &artifact_refs,
                    &started_at,
                    &completed_at,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update automation task for workflow run transition: {error}"
                ))
            })?;
        let task = row_to_stored_automation_task(&task_row)?;

        let task_message = request.message.clone().unwrap_or_else(|| {
            automation_task_default_message_for_run_state(request.state).to_string()
        });
        transaction
            .execute(
                r#"
                INSERT INTO control_automation_task_events (
                    id,
                    task_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &Uuid::now_v7(),
                    &task.id,
                    &automation_task_event_type_for_run_state(request.state),
                    &task_message,
                    &request.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert automation task event for workflow run transition: {error}"
                ))
            })?;

        let run_row = transaction
            .query_one(
                r#"
                UPDATE control_workflow_runs
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &id,
                    &request.state.as_str(),
                    &task.output,
                    &task.error,
                    &json_string_array(&task.artifact_refs),
                    &task.started_at,
                    &task.completed_at,
                    &task.updated_at,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update workflow run state: {error}"))
            })?;

        let run_message = request
            .message
            .unwrap_or_else(|| workflow_run_default_message(request.state).to_string());
        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: workflow_run_event_type(request.state).to_string(),
            message: run_message.clone(),
            data: request.data.clone(),
            created_at: now,
        };
        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_run_events (
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &event.id,
                    &id,
                    &event.event_type,
                    &run_message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow run transition event: {error}"
                ))
            })?;
        let run = row_to_stored_workflow_run(&run_row)?;
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(run))
    }

    pub(super) async fn reconcile_workflow_run_from_task(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let run_row = transaction
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock workflow run for reconciliation: {error}"
                ))
            })?;
        let Some(run_row) = run_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current_run = row_to_stored_workflow_run(&run_row)?;

        let task_row = transaction
            .query_one(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    owner_display_name,
                    display_name,
                    executor,
                    state,
                    session_id,
                    session_source,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    cancel_requested_at,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_automation_tasks
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&current_run.automation_task_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock automation task for workflow run reconciliation: {error}"
                ))
            })?;
        let current_task = row_to_stored_automation_task(&task_row)?;
        if !current_task.state.is_terminal() {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(Some(current_run));
        }

        let target_state: WorkflowRunState = current_task.state.into();
        if current_run.state == target_state
            && current_run.output == current_task.output
            && current_run.error == current_task.error
            && current_run.artifact_refs == current_task.artifact_refs
            && current_run.started_at == current_task.started_at
            && current_run.completed_at == current_task.completed_at
        {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(Some(current_run));
        }

        let now = Utc::now();
        let artifact_refs = json_string_array(&current_task.artifact_refs);
        let run_row = transaction
            .query_one(
                r#"
                UPDATE control_workflow_runs
                SET
                    state = $2,
                    output = $3::jsonb,
                    error = $4,
                    artifact_refs = $5::jsonb,
                    started_at = $6,
                    completed_at = $7,
                    updated_at = $8
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &id,
                    &target_state.as_str(),
                    &current_task.output,
                    &current_task.error,
                    &artifact_refs,
                    &current_task.started_at,
                    &current_task.completed_at,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to reconcile workflow run state from automation task: {error}"
                ))
            })?;

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: workflow_run_event_type(target_state).to_string(),
            message: "workflow run reconciled from terminal automation task state".to_string(),
            data: Some(serde_json::json!({
                "reconciled_from": "automation_task"
            })),
            created_at: now,
        };
        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_run_events (
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &event.id,
                    &id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to append workflow run reconciliation event: {error}"
                ))
            })?;
        let run = row_to_stored_workflow_run(&run_row)?;
        Self::enqueue_workflow_event_deliveries(&transaction, &run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(run))
    }

    pub(super) async fn list_awaiting_input_workflow_runs(
        &self,
    ) -> Result<Vec<StoredWorkflowRun>, SessionStoreError> {
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE state = 'awaiting_input'
                ORDER BY updated_at ASC, id ASC
                "#,
                &[],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list awaiting-input workflow runs: {error}"
                ))
            })?;
        rows.into_iter()
            .map(|row| row_to_stored_workflow_run(&row))
            .collect()
    }

    pub(super) async fn append_workflow_run_log(
        &self,
        id: Uuid,
        request: PersistWorkflowRunLogRequest,
    ) -> Result<Option<StoredWorkflowRunLog>, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                WITH inserted AS (
                    INSERT INTO control_workflow_run_logs (
                        id,
                        run_id,
                        stream,
                        message,
                        created_at
                    )
                    SELECT $2, $1, $3, $4, $5
                    WHERE EXISTS (
                        SELECT 1
                        FROM control_workflow_runs
                        WHERE id = $1
                    )
                    RETURNING
                        id,
                        run_id,
                        stream,
                        message,
                        created_at
                )
                UPDATE control_workflow_runs
                SET updated_at = $5
                WHERE id = $1
                  AND EXISTS (SELECT 1 FROM inserted)
                RETURNING (SELECT id FROM inserted) AS inserted_id
                "#,
                &[
                    &id,
                    &Uuid::now_v7(),
                    &request.stream.as_str(),
                    &request.message,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to append workflow run log: {error}"))
            })?;
        let Some(row) = row else {
            return Ok(None);
        };
        let inserted_id: Option<Uuid> = row.get("inserted_id");
        let Some(inserted_id) = inserted_id else {
            return Ok(None);
        };
        let log_row = self
            .db
            .client()
            .await?
            .query_one(
                r#"
                SELECT
                    id,
                    run_id,
                    stream,
                    message,
                    created_at
                FROM control_workflow_run_logs
                WHERE id = $1
                "#,
                &[&inserted_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to reload workflow run log: {error}"))
            })?;
        row_to_stored_workflow_run_log(&log_row).map(Some)
    }

    pub(super) async fn append_workflow_run_produced_file(
        &self,
        id: Uuid,
        request: PersistWorkflowRunProducedFileRequest,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;

        let run_row = transaction
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                FROM control_workflow_runs
                WHERE id = $1
                FOR UPDATE
                "#,
                &[&id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to lock workflow run for produced file append: {error}"
                ))
            })?;
        let Some(run_row) = run_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };

        let mut run = row_to_stored_workflow_run(&run_row)?;
        if run
            .produced_files
            .iter()
            .any(|file| file.file_id == request.file_id)
        {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::Conflict(format!(
                "workflow run {id} already contains produced file {}",
                request.file_id
            )));
        }

        let now = Utc::now();
        let produced_file = WorkflowRunProducedFile {
            workspace_id: request.workspace_id,
            file_id: request.file_id,
            file_name: request.file_name,
            media_type: request.media_type,
            byte_count: request.byte_count,
            sha256_hex: request.sha256_hex,
            provenance: request.provenance,
            artifact_ref: request.artifact_ref,
            created_at: now,
        };
        run.produced_files.push(produced_file.clone());

        let row = transaction
            .query_one(
                r#"
                UPDATE control_workflow_runs
                SET
                    produced_files = $2::jsonb,
                    updated_at = $3
                WHERE id = $1
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_system,
                    source_reference,
                    client_request_id,
                    create_request_fingerprint,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[
                    &id,
                    &json_workflow_run_produced_files(&run.produced_files)?,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow run produced files: {error}"
                ))
            })?;

        let event = StoredWorkflowRunEvent {
            id: Uuid::now_v7(),
            run_id: id,
            event_type: "workflow_run.produced_file_added".to_string(),
            message: format!(
                "workflow run produced file {} stored in workspace {}",
                produced_file.file_id, produced_file.workspace_id
            ),
            data: Some(serde_json::json!({
                "workspace_id": produced_file.workspace_id,
                "file_id": produced_file.file_id,
                "file_name": produced_file.file_name,
            })),
            created_at: now,
        };
        transaction
            .execute(
                r#"
                INSERT INTO control_workflow_run_events (
                    id,
                    run_id,
                    event_type,
                    message,
                    data,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5::jsonb, $6)
                "#,
                &[
                    &event.id,
                    &id,
                    &event.event_type,
                    &event.message,
                    &event.data,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to insert workflow produced file event: {error}"
                ))
            })?;
        let updated_run = row_to_stored_workflow_run(&row)?;
        Self::enqueue_workflow_event_deliveries(&transaction, &updated_run, &event).await?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        Ok(Some(updated_run))
    }

    pub(super) async fn list_workflow_run_log_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunLogRetentionCandidate>, SessionStoreError> {
        let retention_secs = retention.num_seconds() as f64;
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    run.id AS run_id,
                    run.automation_task_id,
                    run.session_id,
                    run.completed_at
                FROM control_workflow_runs run
                WHERE run.completed_at IS NOT NULL
                  AND EXTRACT(EPOCH FROM ($1 - run.completed_at)) >= $2::DOUBLE PRECISION
                  AND (
                    EXISTS (
                        SELECT 1
                        FROM control_workflow_run_logs logs
                        WHERE logs.run_id = run.id
                    )
                    OR EXISTS (
                        SELECT 1
                        FROM control_automation_task_logs logs
                        WHERE logs.task_id = run.automation_task_id
                    )
                  )
                ORDER BY run.completed_at ASC, run.id ASC
                "#,
                &[&now, &retention_secs],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow run log retention candidates: {}",
                    describe_postgres_error(&error)
                ))
            })?;
        Ok(rows
            .iter()
            .map(|row| {
                let completed_at: DateTime<Utc> = row.get("completed_at");
                WorkflowRunLogRetentionCandidate {
                    run_id: row.get("run_id"),
                    automation_task_id: row.get("automation_task_id"),
                    session_id: row.get("session_id"),
                    expires_at: completed_at + retention,
                }
            })
            .collect())
    }

    pub(super) async fn delete_workflow_run_logs(
        &self,
        run_id: Uuid,
        automation_task_id: Uuid,
    ) -> Result<usize, SessionStoreError> {
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let run_deleted = transaction
            .execute(
                "DELETE FROM control_workflow_run_logs WHERE run_id = $1",
                &[&run_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete workflow run logs for {run_id}: {error}"
                ))
            })? as usize;
        let task_deleted = transaction
            .execute(
                "DELETE FROM control_automation_task_logs WHERE task_id = $1",
                &[&automation_task_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete automation task logs for {automation_task_id}: {error}"
                ))
            })? as usize;
        let now = Utc::now();
        transaction
            .execute(
                "UPDATE control_workflow_runs SET updated_at = $2 WHERE id = $1",
                &[&run_id, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow run after log deletion: {error}"
                ))
            })?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        Ok(run_deleted + task_deleted)
    }

    pub(super) async fn list_workflow_run_output_retention_candidates(
        &self,
        now: DateTime<Utc>,
        retention: ChronoDuration,
    ) -> Result<Vec<WorkflowRunOutputRetentionCandidate>, SessionStoreError> {
        let retention_secs = retention.num_seconds() as f64;
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    run.id AS run_id,
                    run.session_id,
                    run.completed_at
                FROM control_workflow_runs run
                WHERE run.completed_at IS NOT NULL
                  AND run.output IS NOT NULL
                  AND EXTRACT(EPOCH FROM ($1 - run.completed_at)) >= $2::DOUBLE PRECISION
                ORDER BY run.completed_at ASC, run.id ASC
                "#,
                &[&now, &retention_secs],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow run output retention candidates: {}",
                    describe_postgres_error(&error)
                ))
            })?;
        Ok(rows
            .iter()
            .map(|row| {
                let completed_at: DateTime<Utc> = row.get("completed_at");
                WorkflowRunOutputRetentionCandidate {
                    run_id: row.get("run_id"),
                    session_id: row.get("session_id"),
                    expires_at: completed_at + retention,
                }
            })
            .collect())
    }

    pub(super) async fn clear_workflow_run_output(
        &self,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_workflow_runs
                SET
                    output = NULL,
                    updated_at = $2
                WHERE id = $1
                RETURNING
                    id,
                    workflow_definition_id,
                    workflow_definition_version_id,
                    workflow_version,
                    session_id,
                    automation_task_id,
                    state,
                    source_snapshot,
                    extensions,
                    credential_bindings,
                    workspace_inputs,
                    produced_files,
                    input,
                    output,
                    error,
                    artifact_refs,
                    labels,
                    started_at,
                    completed_at,
                    created_at,
                    updated_at
                "#,
                &[&run_id, &Utc::now()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to clear workflow run output: {error}"))
            })?;
        row.as_ref().map(row_to_stored_workflow_run).transpose()
    }
}
