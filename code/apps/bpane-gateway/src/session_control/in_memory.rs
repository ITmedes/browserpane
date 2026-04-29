use super::*;
use std::ops::Deref;

mod automation_tasks;
mod recordings;
mod runtime_assignments;
mod sessions;
mod state;
mod workflow_definitions;
mod workflow_events;
mod workflow_runs;

use state::*;

pub(super) struct InMemorySessionStore {
    state: InMemoryStoreState,
    config: SessionStoreConfig,
}

impl Deref for InMemorySessionStore {
    type Target = InMemoryStoreState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl InMemorySessionStore {
    pub(super) fn new(config: SessionStoreConfig) -> Self {
        Self {
            state: InMemoryStoreState::new(),
            config,
        }
    }

    pub(super) async fn create_credential_binding(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistCredentialBindingRequest,
    ) -> Result<StoredCredentialBinding, SessionStoreError> {
        let now = Utc::now();
        let binding = StoredCredentialBinding {
            id: request.id,
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            provider: request.provider,
            external_ref: request.external_ref,
            namespace: request.namespace,
            allowed_origins: request.allowed_origins,
            injection_mode: request.injection_mode,
            totp: request.totp,
            labels: request.labels,
            created_at: now,
            updated_at: now,
        };
        self.credential_bindings.lock().await.push(binding.clone());
        Ok(binding)
    }

    pub(super) async fn list_credential_bindings_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredCredentialBinding>, SessionStoreError> {
        let mut bindings = self
            .credential_bindings
            .lock()
            .await
            .iter()
            .filter(|binding| {
                binding.owner_subject == principal.subject
                    && binding.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        bindings.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(bindings)
    }

    pub(super) async fn get_credential_binding_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredCredentialBinding>, SessionStoreError> {
        Ok(self
            .credential_bindings
            .lock()
            .await
            .iter()
            .find(|binding| {
                binding.id == id
                    && binding.owner_subject == principal.subject
                    && binding.owner_issuer == principal.issuer
            })
            .cloned())
    }
}
