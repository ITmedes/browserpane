use super::*;

fn active_runtime_candidate_count(sessions: &[StoredSession]) -> usize {
    sessions
        .iter()
        .filter(|session| session.state.is_runtime_candidate())
        .count()
}

fn active_project_session_count(
    sessions: &[StoredSession],
    owner_subject: &str,
    owner_issuer: &str,
    project_id: Uuid,
) -> u32 {
    sessions
        .iter()
        .filter(|session| {
            session.project_id == Some(project_id)
                && session.owner.subject == owner_subject
                && session.owner.issuer == owner_issuer
                && session.state.is_runtime_candidate()
        })
        .count() as u32
}

fn project_session_creation_count(
    sessions: &[StoredSession],
    owner_subject: &str,
    owner_issuer: &str,
    project_id: Uuid,
) -> u32 {
    sessions
        .iter()
        .filter(|session| {
            session.project_id == Some(project_id)
                && session.owner.subject == owner_subject
                && session.owner.issuer == owner_issuer
        })
        .count() as u32
}

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

fn promote_queued_project_sessions(
    sessions: &mut [StoredSession],
    projects: &[StoredProject],
    max_runtime_candidates: usize,
    now: chrono::DateTime<Utc>,
) {
    loop {
        if active_runtime_candidate_count(sessions) >= max_runtime_candidates {
            break;
        }

        let Some((candidate_index, active_sessions, max_active_sessions)) = sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| session.state == SessionLifecycleState::Queued)
            .filter_map(|(index, session)| {
                let project_id = session.project_id?;
                let project = projects.iter().find(|project| {
                    project.id == project_id
                        && project.owner_subject == session.owner.subject
                        && project.owner_issuer == session.owner.issuer
                })?;
                if project.state == ProjectState::Archived {
                    return None;
                }
                let active_sessions = active_project_session_count(
                    sessions,
                    &session.owner.subject,
                    &session.owner.issuer,
                    project_id,
                );
                if project
                    .quotas
                    .max_active_sessions
                    .is_some_and(|max_active_sessions| active_sessions >= max_active_sessions)
                {
                    return None;
                }
                Some((
                    index,
                    active_sessions,
                    project.quotas.max_active_sessions,
                    session.created_at,
                ))
            })
            .min_by_key(|(_, _, _, created_at)| *created_at)
            .map(|(index, active_sessions, max_active_sessions, _)| {
                (index, active_sessions, max_active_sessions)
            })
        else {
            break;
        };

        let session = &mut sessions[candidate_index];
        let Some(project_id) = session.project_id else {
            continue;
        };
        session.state = SessionLifecycleState::Ready;
        session.admission = ProjectAdmissionDecision::project_quota_available(
            project_id,
            active_sessions.saturating_add(1),
            max_active_sessions,
            now,
        );
        session.updated_at = now;
        session.queued_at = None;
        session.runtime_started_at = Some(now);
        session.runtime_released_at = None;
        session.stopped_at = None;
    }
}

fn finalize_runtime_usage(session: &mut StoredSession, now: chrono::DateTime<Utc>) {
    let Some(started_at) = session.runtime_started_at.take() else {
        return;
    };
    let elapsed_ms = now
        .signed_duration_since(started_at)
        .num_milliseconds()
        .max(0) as u64;
    session.runtime_usage_ms = session.runtime_usage_ms.saturating_add(elapsed_ms);
}

impl InMemorySessionStore {
    pub(in crate::session_control) async fn get_session_by_id(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .find(|session| session.id == id)
            .cloned();
        Ok(session)
    }

    pub(in crate::session_control) async fn create_session(
        &self,
        principal: &AuthenticatedPrincipal,
        request: CreateSessionRequest,
        owner_mode: SessionOwnerMode,
    ) -> Result<StoredSession, SessionStoreError> {
        let now = Utc::now();
        let mut sessions = self.sessions.lock().await;
        let (admission, lifecycle_state) = if let Some(project_id) = request.project_id {
            let project = self
                .projects
                .lock()
                .await
                .iter()
                .find(|project| {
                    project.id == project_id
                        && project.owner_subject == principal.subject
                        && project.owner_issuer == principal.issuer
                })
                .cloned()
                .ok_or_else(|| {
                    SessionStoreError::NotFound(format!("project {project_id} not found"))
                })?;
            let active_project_sessions = active_project_session_count(
                &sessions,
                &principal.subject,
                &principal.issuer,
                project_id,
            );
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
            let session_creations = project_session_creation_count(
                &sessions,
                &principal.subject,
                &principal.issuer,
                project_id,
            );
            validate_project_session_creation_budget(&project, session_creations, now)?;
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
        if lifecycle_state.is_runtime_candidate()
            && active_runtime_candidate_count(&sessions) >= self.config.max_runtime_candidates
        {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }
        let browser_context = request.browser_context.unwrap_or_default();
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
        let session = StoredSession {
            id: Uuid::now_v7(),
            state: lifecycle_state,
            project_id: request.project_id,
            admission,
            template_id: request.template_id,
            browser_context: SessionBrowserContextResource {
                mode: browser_context.mode,
                context_id: browser_context.context_id,
            },
            network_identity: request.network_identity.unwrap_or_default(),
            owner_mode,
            viewport: request.viewport.unwrap_or_default(),
            owner: SessionOwner {
                subject: principal.subject.clone(),
                issuer: principal.issuer.clone(),
                display_name: principal.display_name.clone(),
            },
            automation_delegate: None,
            idle_timeout_sec: request.idle_timeout_sec,
            labels: request.labels,
            integration_context: request.integration_context,
            extensions: request.extensions,
            recording: request.recording,
            created_at: now,
            updated_at: now,
            queued_at,
            runtime_started_at,
            runtime_usage_ms: 0,
            egress_rx_bytes: 0,
            egress_tx_bytes: 0,
            runtime_released_at: None,
            stopped_at: None,
        };
        sessions.push(session.clone());
        Ok(session)
    }

    pub(in crate::session_control) async fn list_sessions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSession>, SessionStoreError> {
        let mut sessions = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| {
                session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(sessions)
    }

    pub(in crate::session_control) async fn get_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .find(|session| {
                session.id == id
                    && session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .cloned();
        Ok(session)
    }

    pub(in crate::session_control) async fn get_session_for_principal(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .find(|session| session.id == id && session_visible_to_principal(session, principal))
            .cloned();
        Ok(session)
    }

    pub(in crate::session_control) async fn get_runtime_candidate_session(
        &self,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let session = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| session.state.is_runtime_candidate())
            .max_by(|left, right| left.updated_at.cmp(&right.updated_at))
            .cloned();
        Ok(session)
    }

    pub(in crate::session_control) async fn stop_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        if session.state != SessionLifecycleState::Stopped {
            let now = Utc::now();
            finalize_runtime_usage(session, now);
            session.state = SessionLifecycleState::Stopped;
            session.updated_at = now;
            session.queued_at = None;
            session.runtime_started_at = None;
            session.stopped_at = Some(session.updated_at);
        }

        let stopped = session.clone();
        let now = Utc::now();
        let projects = self.projects.lock().await.clone();
        promote_queued_project_sessions(
            &mut sessions,
            &projects,
            self.config.max_runtime_candidates,
            now,
        );

        Ok(Some(stopped))
    }

    pub(in crate::session_control) async fn cancel_queued_session_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };
        if session.state != SessionLifecycleState::Queued {
            return Err(SessionStoreError::Conflict(format!(
                "session {id} is not queued"
            )));
        }

        session.state = SessionLifecycleState::Stopped;
        session.updated_at = Utc::now();
        session.queued_at = None;
        session.runtime_started_at = None;
        session.stopped_at = Some(session.updated_at);
        Ok(Some(session.clone()))
    }

    pub(in crate::session_control) async fn release_session_runtime_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        if session.state == SessionLifecycleState::Stopped {
            return Err(SessionStoreError::Conflict(format!(
                "session {id} is stopped; create a new session instead of releasing it"
            )));
        }
        if !session.state.is_runtime_candidate() && session.state != SessionLifecycleState::Released
        {
            return Err(SessionStoreError::Conflict(format!(
                "session {id} cannot release a runtime from state {}",
                session.state.as_str()
            )));
        }

        let now = Utc::now();
        finalize_runtime_usage(session, now);
        session.state = SessionLifecycleState::Released;
        session.updated_at = now;
        session.queued_at = None;
        session.runtime_started_at = None;
        session.runtime_released_at = Some(session.updated_at);
        session.stopped_at = None;
        let released = session.clone();
        let now = Utc::now();
        let projects = self.projects.lock().await.clone();
        promote_queued_project_sessions(
            &mut sessions,
            &projects,
            self.config.max_runtime_candidates,
            now,
        );
        Ok(Some(released))
    }

    pub(in crate::session_control) async fn mark_session_state(
        &self,
        id: Uuid,
        state: SessionLifecycleState,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| session.id == id) else {
            return Ok(None);
        };

        if !session.state.is_runtime_candidate() {
            return Ok(Some(session.clone()));
        }
        if session.state == state {
            return Ok(Some(session.clone()));
        }

        session.state = state;
        session.updated_at = Utc::now();
        Ok(Some(session.clone()))
    }

    pub(in crate::session_control) async fn stop_session_if_idle(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| session.id == id) else {
            return Ok(None);
        };

        if !matches!(
            session.state,
            SessionLifecycleState::Ready | SessionLifecycleState::Idle
        ) {
            return Ok(Some(session.clone()));
        }

        let now = Utc::now();
        finalize_runtime_usage(session, now);
        session.state = SessionLifecycleState::Stopped;
        session.updated_at = now;
        session.queued_at = None;
        session.runtime_started_at = None;
        session.stopped_at = Some(session.updated_at);
        let stopped = session.clone();
        let now = Utc::now();
        let projects = self.projects.lock().await.clone();
        promote_queued_project_sessions(
            &mut sessions,
            &projects,
            self.config.max_runtime_candidates,
            now,
        );
        Ok(Some(stopped))
    }

    pub(in crate::session_control) async fn prepare_session_for_connect(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(index) = sessions.iter().position(|session| session.id == id) else {
            return Ok(None);
        };

        let state = sessions[index].state;
        if state == SessionLifecycleState::Queued {
            let now = Utc::now();
            let projects = self.projects.lock().await.clone();
            promote_queued_project_sessions(
                &mut sessions,
                &projects,
                self.config.max_runtime_candidates,
                now,
            );
            return Ok(Some(sessions[index].clone()));
        }
        if state != SessionLifecycleState::Released && state != SessionLifecycleState::Stopped {
            return Ok(Some(sessions[index].clone()));
        }
        let active_runtime_candidates = active_runtime_candidate_count(&sessions);
        if active_runtime_candidates >= self.config.max_runtime_candidates {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let mut project_admission = None;
        if let Some(project_id) = sessions[index].project_id {
            let projects = self.projects.lock().await;
            let project = projects
                .iter()
                .find(|project| {
                    project.id == project_id
                        && project.owner_subject == sessions[index].owner.subject
                        && project.owner_issuer == sessions[index].owner.issuer
                })
                .cloned()
                .ok_or_else(|| {
                    SessionStoreError::NotFound(format!("project {project_id} not found"))
                })?;
            if project.state == ProjectState::Archived {
                return Err(SessionStoreError::Conflict(format!(
                    "project admission rejected: {}: project {project_id} is archived",
                    ProjectAdmissionReasonCode::ProjectArchived.as_str()
                )));
            }
            let active_project_sessions = active_project_session_count(
                &sessions,
                &sessions[index].owner.subject,
                &sessions[index].owner.issuer,
                project_id,
            );
            if let Some(max_active_sessions) = project.quotas.max_active_sessions {
                if active_project_sessions >= max_active_sessions {
                    let now = Utc::now();
                    let session = &mut sessions[index];
                    session.state = SessionLifecycleState::Queued;
                    session.admission = queued_session_admission(
                        project_id,
                        active_project_sessions,
                        max_active_sessions,
                        now,
                    );
                    session.updated_at = now;
                    session.queued_at = Some(now);
                    session.runtime_started_at = None;
                    return Ok(Some(session.clone()));
                }
            }
            project_admission = Some(ProjectAdmissionDecision::project_quota_available(
                project_id,
                active_project_sessions.saturating_add(1),
                project.quotas.max_active_sessions,
                Utc::now(),
            ));
        }

        let session = &mut sessions[index];
        if state == SessionLifecycleState::Stopped {
            session.runtime_released_at = session
                .stopped_at
                .or(session.runtime_released_at)
                .or_else(|| Some(Utc::now()));
        }
        session.state = SessionLifecycleState::Ready;
        session.updated_at = Utc::now();
        session.queued_at = None;
        session.runtime_started_at = Some(session.updated_at);
        session.stopped_at = None;
        if let Some(admission) = project_admission {
            session.admission = admission;
        }
        Ok(Some(session.clone()))
    }

    pub(in crate::session_control) async fn set_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: SetAutomationDelegateRequest,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        session.automation_delegate = Some(SessionAutomationDelegate {
            client_id: request.client_id,
            issuer: request.issuer.unwrap_or_else(|| principal.issuer.clone()),
            display_name: request.display_name,
        });
        session.updated_at = Utc::now();

        Ok(Some(session.clone()))
    }

    pub(in crate::session_control) async fn clear_automation_delegate_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| {
            session.id == id
                && session.owner.subject == principal.subject
                && session.owner.issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        session.automation_delegate = None;
        session.updated_at = Utc::now();

        Ok(Some(session.clone()))
    }
}
