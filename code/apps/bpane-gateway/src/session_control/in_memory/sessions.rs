use super::*;

fn active_runtime_candidate_count(sessions: &[StoredSession]) -> usize {
    sessions
        .iter()
        .filter(|session| session.state.is_runtime_candidate())
        .count()
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
        let mut sessions = self.sessions.lock().await;
        let active_runtime_candidates = active_runtime_candidate_count(&sessions);
        if active_runtime_candidates >= self.config.max_runtime_candidates {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let now = Utc::now();
        let session = StoredSession {
            id: Uuid::now_v7(),
            state: SessionLifecycleState::Ready,
            template_id: request.template_id,
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
            session.state = SessionLifecycleState::Stopped;
            session.updated_at = Utc::now();
            session.stopped_at = Some(session.updated_at);
        }

        Ok(Some(session.clone()))
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

        session.state = SessionLifecycleState::Stopped;
        session.updated_at = Utc::now();
        session.stopped_at = Some(session.updated_at);
        Ok(Some(session.clone()))
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
        if state != SessionLifecycleState::Stopped {
            return Ok(Some(sessions[index].clone()));
        }

        let active_runtime_candidates = active_runtime_candidate_count(&sessions);
        if active_runtime_candidates >= self.config.max_runtime_candidates {
            return Err(SessionStoreError::ActiveSessionConflict {
                max_runtime_sessions: self.config.max_runtime_candidates,
            });
        }

        let session = &mut sessions[index];
        session.state = SessionLifecycleState::Ready;
        session.updated_at = Utc::now();
        session.stopped_at = None;
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
