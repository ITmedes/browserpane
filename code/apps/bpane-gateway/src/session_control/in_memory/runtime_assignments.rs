use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn upsert_runtime_assignment(
        &self,
        assignment: PersistedSessionRuntimeAssignment,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignments
            .lock()
            .await
            .insert(assignment.session_id, assignment);
        Ok(())
    }

    pub(in crate::session_control) async fn clear_runtime_assignment(
        &self,
        id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.runtime_assignments.lock().await.remove(&id);
        Ok(())
    }

    pub(in crate::session_control) async fn upsert_recording_worker_assignment(
        &self,
        assignment: PersistedSessionRecordingWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        self.recording_worker_assignments
            .lock()
            .await
            .insert(assignment.session_id, assignment);
        Ok(())
    }

    pub(in crate::session_control) async fn clear_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.recording_worker_assignments.lock().await.remove(&id);
        Ok(())
    }

    pub(in crate::session_control) async fn get_recording_worker_assignment(
        &self,
        id: Uuid,
    ) -> Result<Option<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        Ok(self
            .recording_worker_assignments
            .lock()
            .await
            .get(&id)
            .cloned())
    }

    pub(in crate::session_control) async fn list_recording_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedSessionRecordingWorkerAssignment>, SessionStoreError> {
        let assignments = self.recording_worker_assignments.lock().await;
        let mut values = assignments.values().cloned().collect::<Vec<_>>();
        values.sort_by_key(|assignment| assignment.session_id);
        Ok(values)
    }

    pub(in crate::session_control) async fn upsert_workflow_run_worker_assignment(
        &self,
        assignment: PersistedWorkflowRunWorkerAssignment,
    ) -> Result<(), SessionStoreError> {
        self.workflow_run_worker_assignments
            .lock()
            .await
            .insert(assignment.run_id, assignment);
        Ok(())
    }

    pub(in crate::session_control) async fn clear_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<(), SessionStoreError> {
        self.workflow_run_worker_assignments
            .lock()
            .await
            .remove(&run_id);
        Ok(())
    }

    pub(in crate::session_control) async fn get_workflow_run_worker_assignment(
        &self,
        run_id: Uuid,
    ) -> Result<Option<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        Ok(self
            .workflow_run_worker_assignments
            .lock()
            .await
            .get(&run_id)
            .cloned())
    }

    pub(in crate::session_control) async fn list_workflow_run_worker_assignments(
        &self,
    ) -> Result<Vec<PersistedWorkflowRunWorkerAssignment>, SessionStoreError> {
        let assignments = self.workflow_run_worker_assignments.lock().await;
        let mut values = assignments.values().cloned().collect::<Vec<_>>();
        values.sort_by_key(|assignment| assignment.run_id);
        Ok(values)
    }

    pub(in crate::session_control) async fn list_runtime_assignments(
        &self,
        runtime_binding: &str,
    ) -> Result<Vec<PersistedSessionRuntimeAssignment>, SessionStoreError> {
        let assignments = self.runtime_assignments.lock().await;
        let mut values = assignments
            .values()
            .filter(|assignment| assignment.runtime_binding == runtime_binding)
            .cloned()
            .collect::<Vec<_>>();
        values.sort_by_key(|assignment| assignment.session_id);
        Ok(values)
    }

    pub(in crate::session_control) async fn mark_session_ready_after_runtime_loss(
        &self,
        id: Uuid,
    ) -> Result<Option<StoredSession>, SessionStoreError> {
        let mut sessions = self.sessions.lock().await;
        let Some(session) = sessions.iter_mut().find(|session| session.id == id) else {
            return Ok(None);
        };

        if session.state.is_runtime_candidate() {
            session.state = SessionLifecycleState::Ready;
            session.updated_at = Utc::now();
        }

        Ok(Some(session.clone()))
    }
}
