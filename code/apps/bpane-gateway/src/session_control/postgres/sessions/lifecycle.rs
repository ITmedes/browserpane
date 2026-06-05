use super::*;

fn queued_session_admission(
    project_id: Uuid,
    active_sessions: u32,
    max_active_sessions: u32,
    now: chrono::DateTime<Utc>,
) -> ProjectAdmissionDecision {
    ProjectAdmissionDecision::session_queued(
        project_id,
        ProjectAdmissionReasonCode::ActiveSessionQuotaExceeded,
        format!(
            "project {project_id} active session quota is exhausted ({active_sessions}/{max_active_sessions}); session queued until capacity is available"
        ),
        active_sessions,
        Some(max_active_sessions),
        now,
    )
}

impl SessionRepository<'_> {
    async fn validate_session_egress_credential_project_scope_in_transaction(
        &self,
        transaction: &Transaction<'_>,
        principal: &AuthenticatedPrincipal,
        request: &CreateSessionRequest,
    ) -> Result<(), SessionStoreError> {
        let Some(egress_profile_id) = request
            .network_identity
            .as_ref()
            .and_then(|identity| identity.egress_profile_id)
        else {
            return Ok(());
        };
        let Some(profile_row) = transaction
            .query_opt(
                r#"
                SELECT proxy
                FROM control_egress_profiles
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&egress_profile_id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load egress profile for credential scope validation: {error}"
                ))
            })?
        else {
            return Ok(());
        };
        let proxy = profile_row
            .get::<_, Option<Value>>("proxy")
            .map(serde_json::from_value::<EgressProxyConfig>)
            .transpose()
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to decode egress profile proxy for credential scope validation: {error}"
                ))
            })?;
        let Some(credential_binding_id) = proxy.and_then(|proxy| proxy.credential_binding_id)
        else {
            return Ok(());
        };
        let binding_row = transaction
            .query_opt(
                r#"
                SELECT project_id
                FROM control_credential_bindings
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &credential_binding_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load credential binding for egress scope validation: {error}"
                ))
            })?
            .ok_or_else(|| {
                SessionStoreError::NotFound(format!(
                    "credential binding {credential_binding_id} not found"
                ))
            })?;
        let binding_project_id = binding_row.get("project_id");
        validate_credential_binding_project_scope(
            request.project_id,
            credential_binding_id,
            binding_project_id,
            "session",
        )
    }

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

        let now = Utc::now();
        let (admission, lifecycle_state) = if let Some(project_id) = request.project_id {
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
            validate_project_session_policy(&project, &request, active_project_sessions, now)?;
            let session_creations = self
                .count_session_creations_for_project_in_transaction(
                    &transaction,
                    principal,
                    project_id,
                )
                .await?;
            validate_project_session_creation_budget(&project, session_creations, now)?;
            if let Some(window_started_at) = project_session_creation_window_start(&project, now) {
                let session_creations_in_window = self
                    .count_session_creations_for_project_since_in_transaction(
                        &transaction,
                        principal,
                        project_id,
                        window_started_at,
                    )
                    .await?;
                validate_project_session_creation_rate(&project, session_creations_in_window, now)?;
            }
            let runtime_usage_ms = self
                .sum_runtime_usage_ms_for_project_in_transaction(
                    &transaction,
                    principal,
                    project_id,
                    now,
                )
                .await?;
            validate_project_runtime_usage_budget(&project, runtime_usage_ms, now)?;
            if let Some(max_active_sessions) = project.quotas.max_active_sessions {
                if active_project_sessions >= max_active_sessions {
                    (
                        queued_session_admission(
                            project_id,
                            active_project_sessions,
                            max_active_sessions,
                            now,
                        ),
                        SessionLifecycleState::Queued,
                    )
                } else {
                    (
                        ProjectAdmissionDecision::project_quota_available(
                            project_id,
                            active_project_sessions.saturating_add(1),
                            project.quotas.max_active_sessions,
                            now,
                        ),
                        SessionLifecycleState::Ready,
                    )
                }
            } else {
                (
                    ProjectAdmissionDecision::project_quota_available(
                        project_id,
                        active_project_sessions.saturating_add(1),
                        project.quotas.max_active_sessions,
                        now,
                    ),
                    SessionLifecycleState::Ready,
                )
            }
        } else {
            (
                ProjectAdmissionDecision::owner_scope_unbounded(now),
                SessionLifecycleState::Ready,
            )
        };
        self.validate_session_egress_credential_project_scope_in_transaction(
            &transaction,
            principal,
            &request,
        )
        .await?;
        if lifecycle_state.is_runtime_candidate() {
            let active_runtime_candidates = self
                .count_active_runtime_candidates_in_transaction(&transaction)
                .await?;
            if active_runtime_candidates >= self.store.config.max_runtime_candidates as i64 {
                return Err(SessionStoreError::ActiveSessionConflict {
                    max_runtime_sessions: self.store.config.max_runtime_candidates,
                });
            }
        }
        let admission_value = serde_json::to_value(&admission).map_err(|error| {
            SessionStoreError::Backend(format!("failed to encode session admission: {error}"))
        })?;
        let queued_at = if lifecycle_state == SessionLifecycleState::Queued {
            Some(now)
        } else {
            None
        };
        let runtime_started_at = if lifecycle_state.is_runtime_candidate() {
            Some(now)
        } else {
            None
        };
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
        let viewport = request.viewport.unwrap_or_default();
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
                updated_at,
                queued_at,
                runtime_started_at,
                runtime_usage_ms,
                egress_rx_bytes,
                egress_tx_bytes
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7::jsonb, $8, $9, $10, $11::jsonb, $12, $13, $14, $15,
                $16::jsonb, $17::jsonb, $18::jsonb, $19::jsonb, $20, $21, $21, $22, $23, 0, 0, 0
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
                    &lifecycle_state.as_str(),
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
                    &queued_at,
                    &runtime_started_at,
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

    async fn promote_queued_project_sessions_in_transaction(
        &self,
        transaction: &Transaction<'_>,
    ) -> Result<(), SessionStoreError> {
        loop {
            let active_runtime_candidates = self
                .count_active_runtime_candidates_in_transaction(transaction)
                .await?;
            if active_runtime_candidates >= self.store.config.max_runtime_candidates as i64 {
                break;
            }

            let query = format!(
                r#"
                SELECT
                    {SESSION_COLUMNS}
                FROM control_sessions
                WHERE runtime_binding = $1
                  AND state = 'queued'
                ORDER BY created_at ASC
                FOR UPDATE
                "#
            );
            let rows = transaction
                .query(&query, &[&self.store.config.runtime_binding])
                .await
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to load queued sessions for admission promotion: {error}"
                    ))
                })?;
            let mut promoted = false;
            for row in rows {
                let queued = row_to_stored_session(&row)?;
                let Some(project_id) = queued.project_id else {
                    continue;
                };
                let principal = AuthenticatedPrincipal {
                    subject: queued.owner.subject.clone(),
                    issuer: queued.owner.issuer.clone(),
                    display_name: queued.owner.display_name.clone(),
                    client_id: None,
                    safe_claims: Default::default(),
                };
                let Some(project) = self
                    .load_project_for_owner_in_transaction(transaction, &principal, project_id)
                    .await?
                else {
                    continue;
                };
                if project.state == ProjectState::Archived {
                    continue;
                }
                let active_project_sessions = self
                    .count_active_sessions_for_project_in_transaction(
                        transaction,
                        &principal,
                        project_id,
                    )
                    .await?;
                if project
                    .quotas
                    .max_active_sessions
                    .is_some_and(|max_active_sessions| {
                        active_project_sessions >= max_active_sessions
                    })
                {
                    continue;
                }

                let now = Utc::now();
                let admission = ProjectAdmissionDecision::project_quota_available(
                    project_id,
                    active_project_sessions.saturating_add(1),
                    project.quotas.max_active_sessions,
                    now,
                );
                let admission_value = serde_json::to_value(&admission).map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to encode promoted session admission: {error}"
                    ))
                })?;
                transaction
                    .execute(
                        r#"
                        UPDATE control_sessions
                        SET
                            state = 'ready',
                            admission = $2::jsonb,
                            updated_at = $3,
                            queued_at = NULL,
                            runtime_started_at = $3,
                            runtime_released_at = NULL,
                            stopped_at = NULL
                        WHERE id = $1
                          AND state = 'queued'
                        "#,
                        &[&queued.id, &admission_value, &now],
                    )
                    .await
                    .map_err(|error| {
                        SessionStoreError::Backend(format!(
                            "failed to promote queued session admission: {error}"
                        ))
                    })?;
                promoted = true;
                break;
            }

            if !promoted {
                break;
            }
        }

        Ok(())
    }

    async fn promote_queued_project_sessions(&self) -> Result<(), SessionStoreError> {
        let mut client = self.store.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        self.promote_queued_project_sessions_in_transaction(&transaction)
            .await?;
        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })
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
                queued_at = NULL,
                runtime_usage_ms = runtime_usage_ms + CASE
                    WHEN runtime_started_at IS NULL THEN 0
                    ELSE GREATEST(0, FLOOR(EXTRACT(EPOCH FROM (NOW() - runtime_started_at)) * 1000))::BIGINT
                END,
                runtime_started_at = NULL,
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

        let stopped = row.as_ref().map(row_to_stored_session).transpose()?;
        if stopped.is_some() {
            self.promote_queued_project_sessions().await?;
        }
        Ok(stopped)
    }

    pub(in crate::session_control) async fn cancel_queued_session_for_owner(
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
                queued_at = NULL,
                runtime_started_at = NULL,
                stopped_at = COALESCE(stopped_at, NOW())
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
              AND state = 'queued'
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
                SessionStoreError::Backend(format!("failed to cancel queued session: {error}"))
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
                queued_at = NULL,
                runtime_usage_ms = runtime_usage_ms + CASE
                    WHEN runtime_started_at IS NULL THEN 0
                    ELSE GREATEST(0, FLOOR(EXTRACT(EPOCH FROM (NOW() - runtime_started_at)) * 1000))::BIGINT
                END,
                runtime_started_at = NULL,
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

        let released = row.as_ref().map(row_to_stored_session).transpose()?;
        if released.is_some() {
            self.promote_queued_project_sessions().await?;
        }
        Ok(released)
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

        let stopped = row.as_ref().map(row_to_stored_session).transpose()?;
        if stopped.is_some() {
            self.promote_queued_project_sessions().await?;
        }
        Ok(stopped)
    }

    pub(in crate::session_control) async fn record_session_egress_usage(
        &self,
        id: Uuid,
        request: ReportSessionEgressUsageRequest,
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
                    "failed to lock session for egress usage report: {error}"
                ))
            })?;
        let Some(current_row) = current_row else {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Ok(None);
        };
        let current = row_to_stored_session(&current_row)?;
        let egress_rx_bytes = checked_session_egress_usage_bytes(
            id,
            current.egress_rx_bytes,
            request.rx_bytes_delta,
            "rx",
        )?;
        let egress_tx_bytes = checked_session_egress_usage_bytes(
            id,
            current.egress_tx_bytes,
            request.tx_bytes_delta,
            "tx",
        )?;
        let now = Utc::now();
        let update_query = format!(
            r#"
            UPDATE control_sessions
            SET
                egress_rx_bytes = $2,
                egress_tx_bytes = $3,
                updated_at = $4
            WHERE id = $1
            RETURNING
                {SESSION_COLUMNS}
            "#
        );
        let row = transaction
            .query_one(
                &update_query,
                &[
                    &id,
                    &(egress_rx_bytes as i64),
                    &(egress_tx_bytes as i64),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update session egress usage counters: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;

        row_to_stored_session(&row).map(Some)
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
                queued_at = NULL,
                runtime_usage_ms = runtime_usage_ms + CASE
                    WHEN runtime_started_at IS NULL THEN 0
                    ELSE GREATEST(0, FLOOR(EXTRACT(EPOCH FROM (NOW() - runtime_started_at)) * 1000))::BIGINT
                END,
                runtime_started_at = NULL,
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
        if current.state == SessionLifecycleState::Queued {
            self.promote_queued_project_sessions_in_transaction(&transaction)
                .await?;
            let refreshed_row =
                transaction
                    .query_one(&load_query, &[&id])
                    .await
                    .map_err(|error| {
                        SessionStoreError::Backend(format!(
                            "failed to reload queued session after admission promotion: {error}"
                        ))
                    })?;
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return row_to_stored_session(&refreshed_row).map(Some);
        }
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

        let mut admission_value = None;
        if let Some(project_id) = current.project_id {
            let principal = AuthenticatedPrincipal {
                subject: current.owner.subject.clone(),
                issuer: current.owner.issuer.clone(),
                display_name: current.owner.display_name.clone(),
                client_id: None,
                safe_claims: Default::default(),
            };
            let project = self
                .load_project_for_owner_in_transaction(&transaction, &principal, project_id)
                .await?
                .ok_or_else(|| {
                    SessionStoreError::NotFound(format!("project {project_id} not found"))
                })?;
            if project.state == ProjectState::Archived {
                return Err(SessionStoreError::Conflict(format!(
                    "project admission rejected: {}: project {project_id} is archived",
                    ProjectAdmissionReasonCode::ProjectArchived.as_str()
                )));
            }
            let active_project_sessions = self
                .count_active_sessions_for_project_in_transaction(
                    &transaction,
                    &principal,
                    project_id,
                )
                .await?;
            if let Some(max_active_sessions) = project.quotas.max_active_sessions {
                if active_project_sessions >= max_active_sessions {
                    let now = Utc::now();
                    let admission = queued_session_admission(
                        project_id,
                        active_project_sessions,
                        max_active_sessions,
                        now,
                    );
                    let admission_value = serde_json::to_value(&admission).map_err(|error| {
                        SessionStoreError::Backend(format!(
                            "failed to encode queued reconnect admission: {error}"
                        ))
                    })?;
                    let queue_query = format!(
                        r#"
                        UPDATE control_sessions
                        SET
                            state = 'queued',
                            admission = $2::jsonb,
                            updated_at = $3,
                            queued_at = $3,
                            runtime_started_at = NULL
                        WHERE id = $1
                        RETURNING
                            {SESSION_COLUMNS}
                        "#
                    );
                    let row = transaction
                        .query_one(&queue_query, &[&id, &admission_value, &now])
                        .await
                        .map_err(|error| {
                            SessionStoreError::Backend(format!(
                                "failed to queue session reconnect: {error}"
                            ))
                        })?;
                    transaction.commit().await.map_err(|error| {
                        SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
                    })?;
                    return row_to_stored_session(&row).map(Some);
                }
            }
            let now = Utc::now();
            admission_value = Some(
                serde_json::to_value(ProjectAdmissionDecision::project_quota_available(
                    project_id,
                    active_project_sessions.saturating_add(1),
                    project.quotas.max_active_sessions,
                    now,
                ))
                .map_err(|error| {
                    SessionStoreError::Backend(format!(
                        "failed to encode reconnect admission: {error}"
                    ))
                })?,
            );
        }

        let update_query = format!(
            r#"
            UPDATE control_sessions
            SET
                state = 'ready',
                admission = COALESCE($2::jsonb, admission),
                updated_at = NOW(),
                queued_at = NULL,
                runtime_started_at = NOW(),
                runtime_released_at = COALESCE(stopped_at, runtime_released_at, NOW()),
                stopped_at = NULL
            WHERE id = $1
            RETURNING
                {SESSION_COLUMNS}
            "#
        );
        let row = transaction
            .query_one(&update_query, &[&id, &admission_value])
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
