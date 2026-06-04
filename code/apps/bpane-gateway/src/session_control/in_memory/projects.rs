use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_project(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistProjectRequest,
    ) -> Result<StoredProject, SessionStoreError> {
        let now = Utc::now();
        let mut projects = self.projects.lock().await;
        if projects.iter().any(|project| {
            project.owner_subject == principal.subject
                && project.owner_issuer == principal.issuer
                && project.name == request.name
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "project {} already exists",
                request.name
            )));
        }
        let project = StoredProject {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            labels: request.labels,
            quotas: request.quotas,
            policy: request.policy,
            state: request.state,
            created_at: now,
            updated_at: now,
        };
        projects.push(project.clone());
        Ok(project)
    }

    pub(in crate::session_control) async fn list_projects_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredProject>, SessionStoreError> {
        let mut projects = self
            .projects
            .lock()
            .await
            .iter()
            .filter(|project| {
                project.owner_subject == principal.subject
                    && project.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        projects.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(projects)
    }

    pub(in crate::session_control) async fn get_project_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredProject>, SessionStoreError> {
        Ok(self
            .projects
            .lock()
            .await
            .iter()
            .find(|project| {
                project.id == id
                    && project.owner_subject == principal.subject
                    && project.owner_issuer == principal.issuer
            })
            .cloned())
    }

    pub(in crate::session_control) async fn update_project_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistProjectRequest,
    ) -> Result<Option<StoredProject>, SessionStoreError> {
        let mut projects = self.projects.lock().await;
        if projects.iter().any(|project| {
            project.id != id
                && project.owner_subject == principal.subject
                && project.owner_issuer == principal.issuer
                && project.name == request.name
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "project {} already exists",
                request.name
            )));
        }
        let Some(project) = projects.iter_mut().find(|project| {
            project.id == id
                && project.owner_subject == principal.subject
                && project.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        project.name = request.name;
        project.description = request.description;
        project.labels = request.labels;
        project.quotas = request.quotas;
        project.policy = request.policy;
        project.state = request.state;
        project.updated_at = Utc::now();
        Ok(Some(project.clone()))
    }

    pub(in crate::session_control) async fn count_active_sessions_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        let count = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| {
                session.project_id == Some(project_id)
                    && session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
                    && session.state.is_runtime_candidate()
            })
            .count();
        u32::try_from(count).map_err(|error| {
            SessionStoreError::Backend(format!(
                "active project session count exceeded u32 range: {error}"
            ))
        })
    }

    pub(in crate::session_control) async fn count_queued_sessions_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        let count = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| {
                session.project_id == Some(project_id)
                    && session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
                    && session.state == SessionLifecycleState::Queued
            })
            .count();
        u32::try_from(count).map_err(|error| {
            SessionStoreError::Backend(format!(
                "queued project session count exceeded u32 range: {error}"
            ))
        })
    }

    pub(in crate::session_control) async fn count_session_creations_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        let count = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| {
                session.project_id == Some(project_id)
                    && session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .count();
        u32::try_from(count).map_err(|error| {
            SessionStoreError::Backend(format!(
                "project session creation count exceeded u32 range: {error}"
            ))
        })
    }

    pub(in crate::session_control) async fn count_active_workflow_runs_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        let count = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter(|run| {
                run.project_id == Some(project_id)
                    && run.owner_subject == principal.subject
                    && run.owner_issuer == principal.issuer
                    && run.state.consumes_project_active_quota()
            })
            .count();
        u32::try_from(count).map_err(|error| {
            SessionStoreError::Backend(format!(
                "active project workflow run count exceeded u32 range: {error}"
            ))
        })
    }

    pub(in crate::session_control) async fn sum_runtime_usage_ms_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
        observed_at: DateTime<Utc>,
    ) -> Result<u64, SessionStoreError> {
        self.sessions
            .lock()
            .await
            .iter()
            .filter(|session| {
                session.project_id == Some(project_id)
                    && session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .try_fold(0_u64, |total, session| {
                let live_runtime_ms = if session.state.is_runtime_candidate() {
                    session
                        .runtime_started_at
                        .map(|started_at| {
                            observed_at
                                .signed_duration_since(started_at)
                                .num_milliseconds()
                                .max(0) as u64
                        })
                        .unwrap_or(0)
                } else {
                    0
                };
                total
                    .checked_add(session.runtime_usage_ms)
                    .and_then(|value| value.checked_add(live_runtime_ms))
                    .ok_or_else(|| {
                        SessionStoreError::Backend(
                            "project runtime usage milliseconds exceeded u64 range".to_string(),
                        )
                    })
            })
    }

    pub(in crate::session_control) async fn sum_egress_usage_bytes_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<(u64, u64), SessionStoreError> {
        self.sessions
            .lock()
            .await
            .iter()
            .filter(|session| {
                session.project_id == Some(project_id)
                    && session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .try_fold((0_u64, 0_u64), |(rx_total, tx_total), session| {
                let rx_total = rx_total
                    .checked_add(session.egress_rx_bytes)
                    .ok_or_else(|| {
                        SessionStoreError::Backend(
                            "project egress receive byte count exceeded u64 range".to_string(),
                        )
                    })?;
                let tx_total = tx_total
                    .checked_add(session.egress_tx_bytes)
                    .ok_or_else(|| {
                        SessionStoreError::Backend(
                            "project egress transmit byte count exceeded u64 range".to_string(),
                        )
                    })?;
                Ok((rx_total, tx_total))
            })
    }

    pub(in crate::session_control) async fn sum_retained_storage_bytes_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u64, SessionStoreError> {
        let project_session_ids = self
            .sessions
            .lock()
            .await
            .iter()
            .filter(|session| {
                session.project_id == Some(project_id)
                    && session.owner.subject == principal.subject
                    && session.owner.issuer == principal.issuer
            })
            .map(|session| session.id)
            .collect::<std::collections::HashSet<_>>();

        let workflow_bytes = self
            .workflow_runs
            .lock()
            .await
            .iter()
            .filter(|run| {
                run.project_id == Some(project_id)
                    && run.owner_subject == principal.subject
                    && run.owner_issuer == principal.issuer
            })
            .flat_map(|run| run.produced_files.iter().map(|file| file.byte_count))
            .try_fold(0_u64, checked_retained_storage_add)?;

        let recording_bytes = self
            .recordings
            .lock()
            .await
            .iter()
            .filter(|recording| {
                project_session_ids.contains(&recording.session_id)
                    && recording.artifact_ref.is_some()
            })
            .filter_map(|recording| recording.bytes)
            .try_fold(0_u64, checked_retained_storage_add)?;

        let session_file_bytes = self
            .session_files
            .lock()
            .await
            .iter()
            .filter(|file| project_session_ids.contains(&file.session_id))
            .map(|file| file.byte_count)
            .try_fold(0_u64, checked_retained_storage_add)?;

        workflow_bytes
            .checked_add(recording_bytes)
            .and_then(|total| total.checked_add(session_file_bytes))
            .ok_or_else(|| {
                SessionStoreError::Backend(
                    "project retained storage byte count exceeded u64 range".to_string(),
                )
            })
    }
}

fn checked_retained_storage_add(left: u64, right: u64) -> Result<u64, SessionStoreError> {
    left.checked_add(right).ok_or_else(|| {
        SessionStoreError::Backend(
            "project retained storage byte count exceeded u64 range".to_string(),
        )
    })
}
