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
}
