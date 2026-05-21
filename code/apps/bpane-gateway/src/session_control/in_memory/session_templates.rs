use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_session_template(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistSessionTemplateRequest,
    ) -> Result<StoredSessionTemplate, SessionStoreError> {
        let now = Utc::now();
        let mut templates = self.session_templates.lock().await;
        if templates.iter().any(|template| {
            template.owner_subject == principal.subject
                && template.owner_issuer == principal.issuer
                && template.name == request.name
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "session template {} already exists",
                request.name
            )));
        }
        let template = StoredSessionTemplate {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            labels: request.labels,
            defaults: request.defaults,
            version: 1,
            created_at: now,
            updated_at: now,
        };
        templates.push(template.clone());
        Ok(template)
    }

    pub(in crate::session_control) async fn list_session_templates_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredSessionTemplate>, SessionStoreError> {
        let mut templates = self
            .session_templates
            .lock()
            .await
            .iter()
            .filter(|template| {
                template.owner_subject == principal.subject
                    && template.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        templates.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(templates)
    }

    pub(in crate::session_control) async fn get_session_template_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredSessionTemplate>, SessionStoreError> {
        Ok(self
            .session_templates
            .lock()
            .await
            .iter()
            .find(|template| {
                template.id == id
                    && template.owner_subject == principal.subject
                    && template.owner_issuer == principal.issuer
            })
            .cloned())
    }

    pub(in crate::session_control) async fn update_session_template_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistSessionTemplateRequest,
    ) -> Result<Option<StoredSessionTemplate>, SessionStoreError> {
        let mut templates = self.session_templates.lock().await;
        if templates.iter().any(|template| {
            template.id != id
                && template.owner_subject == principal.subject
                && template.owner_issuer == principal.issuer
                && template.name == request.name
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "session template {} already exists",
                request.name
            )));
        }
        let Some(template) = templates.iter_mut().find(|template| {
            template.id == id
                && template.owner_subject == principal.subject
                && template.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };

        template.name = request.name;
        template.description = request.description;
        template.labels = request.labels;
        template.defaults = request.defaults;
        template.version = template.version.saturating_add(1);
        template.updated_at = Utc::now();
        Ok(Some(template.clone()))
    }
}
