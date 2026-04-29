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

impl PostgresSessionStore {
    pub(super) async fn create_extension_definition(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionDefinitionRequest,
    ) -> Result<StoredExtensionDefinition, SessionStoreError> {
        let now = Utc::now();
        let row = self
            .db
            .client()
            .await?
            .query_one(
                r#"
                INSERT INTO control_extensions (
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                )
                VALUES ($1, $2, $3, $4, $5, TRUE, NULL, $6::jsonb, $7, $7)
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.description,
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to create extension: {error}"))
            })?;
        row_to_stored_extension_definition(&row)
    }

    pub(super) async fn list_extension_definitions_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredExtensionDefinition>, SessionStoreError> {
        let rows = self
            .db
            .client()
            .await?
            .query(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                FROM control_extensions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                ORDER BY created_at DESC
                "#,
                &[&principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list extensions: {error}"))
            })?;
        rows.iter()
            .map(row_to_stored_extension_definition)
            .collect()
    }

    pub(super) async fn get_extension_definition_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                FROM control_extensions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[&id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch extension: {error}"))
            })?;
        row.map(|row| row_to_stored_extension_definition(&row))
            .transpose()
    }

    pub(super) async fn set_extension_definition_enabled_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        enabled: bool,
    ) -> Result<Option<StoredExtensionDefinition>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                UPDATE control_extensions
                SET enabled = $4, updated_at = NOW()
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                RETURNING
                    id,
                    owner_subject,
                    owner_issuer,
                    name,
                    description,
                    enabled,
                    latest_version,
                    labels,
                    created_at,
                    updated_at
                "#,
                &[&id, &principal.subject, &principal.issuer, &enabled],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update extension: {error}"))
            })?;
        row.map(|row| row_to_stored_extension_definition(&row))
            .transpose()
    }

    pub(super) async fn create_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistExtensionVersionRequest,
    ) -> Result<StoredExtensionVersion, SessionStoreError> {
        let mut client = self.db.client().await?;
        let transaction = client.build_transaction().start().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to start transaction: {error}"))
        })?;
        let definition = transaction
            .query_opt(
                r#"
                SELECT id
                FROM control_extensions
                WHERE id = $1
                  AND owner_subject = $2
                  AND owner_issuer = $3
                "#,
                &[
                    &request.extension_definition_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to validate extension: {error}"))
            })?;
        if definition.is_none() {
            transaction.commit().await.map_err(|error| {
                SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
            })?;
            return Err(SessionStoreError::NotFound(format!(
                "extension {} not found",
                request.extension_definition_id
            )));
        }

        let now = Utc::now();
        let row = transaction
            .query_one(
                r#"
                INSERT INTO control_extension_versions (
                    id,
                    extension_definition_id,
                    version,
                    install_path,
                    created_at
                )
                VALUES ($1, $2, $3, $4, $5)
                RETURNING
                    id,
                    extension_definition_id,
                    version,
                    install_path,
                    created_at
                "#,
                &[
                    &Uuid::now_v7(),
                    &request.extension_definition_id,
                    &request.version,
                    &request.install_path,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "extension {} already has version {}",
                        request.extension_definition_id, request.version
                    ));
                }
                SessionStoreError::Backend(format!("failed to create extension version: {error}"))
            })?;

        transaction
            .execute(
                r#"
                UPDATE control_extensions
                SET latest_version = $2, updated_at = $3
                WHERE id = $1
                "#,
                &[&request.extension_definition_id, &request.version, &now],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update extension latest_version: {error}"
                ))
            })?;

        transaction.commit().await.map_err(|error| {
            SessionStoreError::Backend(format!("failed to commit transaction: {error}"))
        })?;
        row_to_stored_extension_version(&row)
    }

    pub(super) async fn get_latest_extension_version_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        extension_definition_id: Uuid,
    ) -> Result<Option<StoredExtensionVersion>, SessionStoreError> {
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT
                    version.id,
                    version.extension_definition_id,
                    version.version,
                    version.install_path,
                    version.created_at
                FROM control_extension_versions version
                JOIN control_extensions extension
                  ON extension.id = version.extension_definition_id
                WHERE version.extension_definition_id = $1
                  AND extension.owner_subject = $2
                  AND extension.owner_issuer = $3
                ORDER BY version.created_at DESC, version.id DESC
                LIMIT 1
                "#,
                &[
                    &extension_definition_id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to fetch latest extension version: {error}"
                ))
            })?;
        row.map(|row| row_to_stored_extension_version(&row))
            .transpose()
    }
}
