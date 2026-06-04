use super::*;

const PROJECT_COLUMNS: &str = r#"
    id,
    owner_subject,
    owner_issuer,
    name,
    description,
    labels,
    quotas,
    policy,
    state,
    created_at,
    updated_at
"#;

pub(super) struct ProjectRepository<'a> {
    store: &'a PostgresSessionStore,
}

impl PostgresSessionStore {
    fn project_repository(&self) -> ProjectRepository<'_> {
        ProjectRepository { store: self }
    }

    pub(in crate::session_control) async fn create_project(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistProjectRequest,
    ) -> Result<StoredProject, SessionStoreError> {
        self.project_repository()
            .create_project(principal, request)
            .await
    }

    pub(in crate::session_control) async fn list_projects_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredProject>, SessionStoreError> {
        self.project_repository()
            .list_projects_for_owner(principal)
            .await
    }

    pub(in crate::session_control) async fn get_project_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredProject>, SessionStoreError> {
        self.project_repository()
            .get_project_for_owner(principal, id)
            .await
    }

    pub(in crate::session_control) async fn update_project_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistProjectRequest,
    ) -> Result<Option<StoredProject>, SessionStoreError> {
        self.project_repository()
            .update_project_for_owner(principal, id, request)
            .await
    }

    pub(in crate::session_control) async fn count_active_sessions_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        self.project_repository()
            .count_active_sessions_for_project(principal, project_id)
            .await
    }

    pub(in crate::session_control) async fn count_active_workflow_runs_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        self.project_repository()
            .count_active_workflow_runs_for_project(principal, project_id)
            .await
    }
}

impl ProjectRepository<'_> {
    async fn create_project(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistProjectRequest,
    ) -> Result<StoredProject, SessionStoreError> {
        let now = Utc::now();
        let quotas_value = serde_json::to_value(&request.quotas).map_err(|error| {
            SessionStoreError::Backend(format!("failed to encode project quotas: {error}"))
        })?;
        let policy_value = serde_json::to_value(&request.policy).map_err(|error| {
            SessionStoreError::Backend(format!("failed to encode project policy: {error}"))
        })?;
        let query = format!(
            r#"
            INSERT INTO control_projects (
                id,
                owner_subject,
                owner_issuer,
                name,
                description,
                labels,
                quotas,
                policy,
                state,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7::jsonb, $8::jsonb, $9, $10, $10)
            RETURNING
                {PROJECT_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_one(
                &query,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.description,
                    &json_labels(&request.labels),
                    &quotas_value,
                    &policy_value,
                    &request.state.as_str(),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "project {} already exists",
                        request.name
                    ));
                }
                SessionStoreError::Backend(format!("failed to create project: {error}"))
            })?;
        row_to_stored_project(&row)
    }

    async fn list_projects_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
    ) -> Result<Vec<StoredProject>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {PROJECT_COLUMNS}
            FROM control_projects
            WHERE owner_subject = $1
              AND owner_issuer = $2
            ORDER BY created_at DESC
            "#
        );
        let rows = self
            .store
            .db
            .client()
            .await?
            .query(&query, &[&principal.subject, &principal.issuer])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list projects: {error}"))
            })?;
        rows.iter().map(row_to_stored_project).collect()
    }

    async fn get_project_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
    ) -> Result<Option<StoredProject>, SessionStoreError> {
        let query = format!(
            r#"
            SELECT
                {PROJECT_COLUMNS}
            FROM control_projects
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(&query, &[&id, &principal.subject, &principal.issuer])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to fetch project: {error}"))
            })?;
        row.as_ref().map(row_to_stored_project).transpose()
    }

    async fn update_project_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        id: Uuid,
        request: PersistProjectRequest,
    ) -> Result<Option<StoredProject>, SessionStoreError> {
        let quotas_value = serde_json::to_value(&request.quotas).map_err(|error| {
            SessionStoreError::Backend(format!("failed to encode project quotas: {error}"))
        })?;
        let policy_value = serde_json::to_value(&request.policy).map_err(|error| {
            SessionStoreError::Backend(format!("failed to encode project policy: {error}"))
        })?;
        let query = format!(
            r#"
            UPDATE control_projects
            SET
                name = $4,
                description = $5,
                labels = $6::jsonb,
                quotas = $7::jsonb,
                policy = $8::jsonb,
                state = $9,
                updated_at = NOW()
            WHERE id = $1
              AND owner_subject = $2
              AND owner_issuer = $3
            RETURNING
                {PROJECT_COLUMNS}
            "#
        );
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                &query,
                &[
                    &id,
                    &principal.subject,
                    &principal.issuer,
                    &request.name,
                    &request.description,
                    &json_labels(&request.labels),
                    &quotas_value,
                    &policy_value,
                    &request.state.as_str(),
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "project {} already exists",
                        request.name
                    ));
                }
                SessionStoreError::Backend(format!("failed to update project: {error}"))
            })?;
        row.as_ref().map(row_to_stored_project).transpose()
    }

    async fn count_active_sessions_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS session_count
                FROM control_sessions
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                  AND project_id = $3
                  AND state IN ('pending', 'starting', 'ready', 'active', 'idle')
                "#,
                &[&principal.subject, &principal.issuer, &project_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to count project active sessions: {error}"
                ))
            })?;
        let count = row
            .as_ref()
            .map(|row| row.get::<_, i64>("session_count"))
            .unwrap_or(0);
        u32::try_from(count).map_err(|error| {
            SessionStoreError::Backend(format!(
                "active project session count exceeded u32 range: {error}"
            ))
        })
    }

    async fn count_active_workflow_runs_for_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<u32, SessionStoreError> {
        let row = self
            .store
            .db
            .client()
            .await?
            .query_opt(
                r#"
                SELECT COUNT(*)::BIGINT AS run_count
                FROM control_workflow_runs
                WHERE owner_subject = $1
                  AND owner_issuer = $2
                  AND project_id = $3
                  AND state IN ('pending', 'starting', 'running', 'awaiting_input')
                "#,
                &[&principal.subject, &principal.issuer, &project_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to count project active workflow runs: {error}"
                ))
            })?;
        let count = row
            .as_ref()
            .map(|row| row.get::<_, i64>("run_count"))
            .unwrap_or(0);
        u32::try_from(count).map_err(|error| {
            SessionStoreError::Backend(format!(
                "active project workflow run count exceeded u32 range: {error}"
            ))
        })
    }
}
