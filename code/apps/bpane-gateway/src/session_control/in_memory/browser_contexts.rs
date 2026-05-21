use super::*;

impl InMemorySessionStore {
    pub(in crate::session_control) async fn create_browser_context(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistBrowserContextRequest,
    ) -> Result<StoredBrowserContext, SessionStoreError> {
        let now = Utc::now();
        let mut contexts = self.browser_contexts.lock().await;
        if contexts.iter().any(|context| {
            context.owner_subject == principal.subject
                && context.owner_issuer == principal.issuer
                && context.name == request.name
        }) {
            return Err(SessionStoreError::Conflict(format!(
                "browser context {} already exists",
                request.name
            )));
        }
        let context = StoredBrowserContext {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            labels: request.labels,
            persistence_mode: request.persistence_mode,
            retention_sec: request.retention_sec,
            state: BrowserContextState::Ready,
            created_at: now,
            updated_at: now,
            last_used_at: None,
            deleted_at: None,
        };
        contexts.push(context.clone());
        Ok(context)
    }

    pub(in crate::session_control) async fn list_browser_contexts_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredBrowserContext>, SessionStoreError> {
        let mut contexts = self
            .browser_contexts
            .lock()
            .await
            .iter()
            .filter(|context| {
                context.owner_subject == principal.subject
                    && context.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        contexts.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(contexts)
    }

    pub(in crate::session_control) async fn get_browser_context_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        Ok(self
            .browser_contexts
            .lock()
            .await
            .iter()
            .find(|context| {
                context.id == id
                    && context.owner_subject == principal.subject
                    && context.owner_issuer == principal.issuer
            })
            .cloned())
    }

    pub(in crate::session_control) async fn mark_browser_context_used_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        let mut contexts = self.browser_contexts.lock().await;
        let Some(context) = contexts.iter_mut().find(|context| {
            context.id == id
                && context.owner_subject == principal.subject
                && context.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };
        let now = Utc::now();
        context.last_used_at = Some(now);
        context.updated_at = now;
        Ok(Some(context.clone()))
    }

    pub(in crate::session_control) async fn delete_browser_context_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredBrowserContext>, SessionStoreError> {
        let mut contexts = self.browser_contexts.lock().await;
        let Some(context) = contexts.iter_mut().find(|context| {
            context.id == id
                && context.owner_subject == principal.subject
                && context.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };
        if context.state != BrowserContextState::Deleted {
            let now = Utc::now();
            context.state = BrowserContextState::Deleted;
            context.updated_at = now;
            context.deleted_at = Some(now);
        }
        Ok(Some(context.clone()))
    }
}
