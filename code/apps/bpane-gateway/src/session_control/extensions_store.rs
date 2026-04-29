use super::*;

impl InMemorySessionStore {
    pub(super) async fn create_extension_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionDefinitionRequest,
    ) -> Result<StoredExtensionDefinition, SessionStoreError> {
        let now = Utc::now();
        let definition = StoredExtensionDefinition {
            id: Uuid::now_v7(),
            owner_subject: principal.subject.clone(),
            owner_issuer: principal.issuer.clone(),
            name: request.name,
            description: request.description,
            enabled: true,
            latest_version: None,
            labels: request.labels,
            created_at: now,
            updated_at: now,
        };
        self.extension_definitions
            .lock()
            .await
            .push(definition.clone());
        Ok(definition)
    }

    pub(super) async fn list_extension_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredExtensionDefinition>, SessionStoreError> {
        let mut definitions = self
            .extension_definitions
            .lock()
            .await
            .iter()
            .filter(|definition| {
                definition.owner_subject == principal.subject
                    && definition.owner_issuer == principal.issuer
            })
            .cloned()
            .collect::<Vec<_>>();
        definitions.sort_by(|left, right| right.created_at.cmp(&left.created_at));
        Ok(definitions)
    }

    pub(super) async fn get_extension_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        Ok(self
            .extension_definitions
            .lock()
            .await
            .iter()
            .find(|definition| {
                definition.id == id
                    && definition.owner_subject == principal.subject
                    && definition.owner_issuer == principal.issuer
            })
            .cloned())
    }

    pub(super) async fn set_extension_definition_enabled_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        enabled: bool,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        let mut definitions = self.extension_definitions.lock().await;
        let Some(definition) = definitions.iter_mut().find(|definition| {
            definition.id == id
                && definition.owner_subject == principal.subject
                && definition.owner_issuer == principal.issuer
        }) else {
            return Ok(None);
        };
        definition.enabled = enabled;
        definition.updated_at = Utc::now();
        Ok(Some(definition.clone()))
    }

    pub(super) async fn create_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionVersionRequest,
    ) -> Result<StoredExtensionVersion, SessionStoreError> {
        let mut definitions = self.extension_definitions.lock().await;
        let Some(definition) = definitions.iter_mut().find(|definition| {
            definition.id == request.extension_definition_id
                && definition.owner_subject == principal.subject
                && definition.owner_issuer == principal.issuer
        }) else {
            return Err(SessionStoreError::InvalidRequest(format!(
                "extension {} not found",
                request.extension_definition_id
            )));
        };
        let versions = self.extension_versions.lock().await;
        if versions.iter().any(|version| {
            version.extension_definition_id == request.extension_definition_id
                && version.version == request.version
        }) {
            return Err(SessionStoreError::InvalidRequest(format!(
                "extension {} already has version {}",
                request.extension_definition_id, request.version
            )));
        }
        drop(versions);
        let now = Utc::now();
        let version = StoredExtensionVersion {
            id: Uuid::now_v7(),
            extension_definition_id: request.extension_definition_id,
            version: request.version,
            install_path: request.install_path,
            created_at: now,
        };
        self.extension_versions.lock().await.push(version.clone());
        definition.latest_version = Some(version.version.clone());
        definition.updated_at = now;
        Ok(version)
    }

    pub(super) async fn get_latest_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        extension_definition_id: Uuid,
    ) -> Result<Option<StoredExtensionVersion>, SessionStoreError> {
        let definitions = self.extension_definitions.lock().await;
        if !definitions.iter().any(|definition| {
            definition.id == extension_definition_id
                && definition.owner_subject == principal.subject
                && definition.owner_issuer == principal.issuer
        }) {
            return Ok(None);
        }
        drop(definitions);
        let latest = self
            .extension_versions
            .lock()
            .await
            .iter()
            .filter(|version| version.extension_definition_id == extension_definition_id)
            .cloned()
            .max_by(|left, right| {
                left.created_at
                    .cmp(&right.created_at)
                    .then_with(|| left.id.cmp(&right.id))
            });
        Ok(latest)
    }
}
