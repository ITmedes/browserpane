use super::*;

const ENDPOINT_COLUMNS: &str = r#"
    id, owner_subject, owner_issuer, project_id, endpoint_key, purpose,
    workflow_definition_id, workflow_definition_version_id, workflow_version,
    input_schema, output_schema, execution_timeout_seconds,
    inline_result_max_bytes, artifact_behavior, labels, state, created_at, updated_at
"#;

const GRANT_COLUMNS: &str = r#"
    id, endpoint_id, project_id, service_principal_id, operations, created_at, updated_at
"#;

const INVOCATION_COLUMNS: &str = r#"
    id, endpoint_id, caller_service_principal_id, idempotency_key,
    request_fingerprint, run_id, outcome, side_effect_state, created_at, updated_at
"#;

impl PostgresSessionStore {
    pub(in crate::session_control) async fn create_workflow_endpoint(
        &self,
        principal: &AuthenticatedPrincipal,
        request: PersistWorkflowEndpointRequest,
    ) -> Result<StoredWorkflowEndpoint, SessionStoreError> {
        let client = self.db.client().await?;
        let binding_exists = client
            .query_opt(
                r#"
                SELECT project.id
                FROM control_projects project
                JOIN control_workflow_definitions workflow
                  ON workflow.id = $4
                 AND workflow.owner_subject = project.owner_subject
                 AND workflow.owner_issuer = project.owner_issuer
                JOIN control_workflow_definition_versions version
                  ON version.id = $5
                 AND version.workflow_definition_id = workflow.id
                 AND version.version = $6
                WHERE project.id = $1
                  AND project.owner_subject = $2
                  AND project.owner_issuer = $3
                "#,
                &[
                    &request.project_id,
                    &principal.subject,
                    &principal.issuer,
                    &request.workflow_definition_id,
                    &request.workflow_definition_version_id,
                    &request.workflow_version,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to validate workflow endpoint binding: {error}"
                ))
            })?
            .is_some();
        if !binding_exists {
            return Err(SessionStoreError::NotFound(
                "workflow endpoint project or immutable workflow version was not found".to_string(),
            ));
        }
        let now = Utc::now();
        let query = format!(
            r#"
            INSERT INTO control_workflow_endpoints (
                id, owner_subject, owner_issuer, project_id, endpoint_key, purpose,
                workflow_definition_id, workflow_definition_version_id, workflow_version,
                input_schema, output_schema, execution_timeout_seconds,
                inline_result_max_bytes, artifact_behavior, labels, state, created_at, updated_at
            )
            VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9,
                $10::jsonb, $11::jsonb, $12, $13, $14::jsonb, $15::jsonb,
                'draft', $16, $16
            )
            RETURNING {ENDPOINT_COLUMNS}
            "#
        );
        let artifact_behavior =
            serde_json::to_value(&request.artifact_behavior).map_err(|error| {
                SessionStoreError::InvalidRequest(format!(
                    "failed to encode workflow endpoint artifact behavior: {error}"
                ))
            })?;
        let row = client
            .query_one(
                &query,
                &[
                    &Uuid::now_v7(),
                    &principal.subject,
                    &principal.issuer,
                    &request.project_id,
                    &request.endpoint_key,
                    &request.purpose,
                    &request.workflow_definition_id,
                    &request.workflow_definition_version_id,
                    &request.workflow_version,
                    &request.input_schema,
                    &request.output_schema,
                    &(request.execution_timeout_seconds as i32),
                    &(request.inline_result_max_bytes as i32),
                    &artifact_behavior,
                    &json_labels(&request.labels),
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                if error.code().is_some_and(|code| code.code() == "23505") {
                    return SessionStoreError::Conflict(format!(
                        "workflow endpoint {} already exists in project {}",
                        request.endpoint_key, request.project_id
                    ));
                }
                SessionStoreError::Backend(format!("failed to create workflow endpoint: {error}"))
            })?;
        row_to_endpoint(&row)
    }

    pub(in crate::session_control) async fn list_workflow_endpoints_for_owner_project(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
    ) -> Result<Vec<StoredWorkflowEndpoint>, SessionStoreError> {
        let query = format!(
            "SELECT {ENDPOINT_COLUMNS} FROM control_workflow_endpoints WHERE owner_subject = $1 AND owner_issuer = $2 AND project_id = $3 ORDER BY endpoint_key"
        );
        let rows = self
            .db
            .client()
            .await?
            .query(
                &query,
                &[&principal.subject, &principal.issuer, &project_id],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to list workflow endpoints: {error}"))
            })?;
        rows.iter().map(row_to_endpoint).collect()
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_for_owner_project_key(
        &self,
        principal: &AuthenticatedPrincipal,
        project_id: Uuid,
        endpoint_key: &str,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        let query = format!(
            "SELECT {ENDPOINT_COLUMNS} FROM control_workflow_endpoints WHERE owner_subject = $1 AND owner_issuer = $2 AND project_id = $3 AND endpoint_key = $4"
        );
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                &query,
                &[
                    &principal.subject,
                    &principal.issuer,
                    &project_id,
                    &endpoint_key,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to get workflow endpoint: {error}"))
            })?;
        row.as_ref().map(row_to_endpoint).transpose()
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_by_project_key(
        &self,
        project_id: Uuid,
        endpoint_key: &str,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        let query = format!(
            "SELECT {ENDPOINT_COLUMNS} FROM control_workflow_endpoints WHERE project_id = $1 AND endpoint_key = $2"
        );
        let row = self
            .db
            .client()
            .await?
            .query_opt(&query, &[&project_id, &endpoint_key])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to get workflow endpoint by project key: {error}"
                ))
            })?;
        row.as_ref().map(row_to_endpoint).transpose()
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_by_id(
        &self,
        endpoint_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        let query =
            format!("SELECT {ENDPOINT_COLUMNS} FROM control_workflow_endpoints WHERE id = $1");
        let row = self
            .db
            .client()
            .await?
            .query_opt(&query, &[&endpoint_id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to get workflow endpoint by id: {error}"
                ))
            })?;
        row.as_ref().map(row_to_endpoint).transpose()
    }

    pub(in crate::session_control) async fn update_workflow_endpoint_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint_id: Uuid,
        request: PersistWorkflowEndpointRequest,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        let artifact_behavior =
            serde_json::to_value(&request.artifact_behavior).map_err(|error| {
                SessionStoreError::InvalidRequest(format!(
                    "failed to encode workflow endpoint artifact behavior: {error}"
                ))
            })?;
        let query = format!(
            r#"
            UPDATE control_workflow_endpoints
            SET purpose = $4, workflow_definition_id = $5,
                workflow_definition_version_id = $6, workflow_version = $7,
                input_schema = $8::jsonb, output_schema = $9::jsonb,
                execution_timeout_seconds = $10, inline_result_max_bytes = $11,
                artifact_behavior = $12::jsonb, labels = $13::jsonb, updated_at = NOW()
            WHERE id = $1 AND owner_subject = $2 AND owner_issuer = $3 AND state <> 'active'
            RETURNING {ENDPOINT_COLUMNS}
            "#
        );
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                &query,
                &[
                    &endpoint_id,
                    &principal.subject,
                    &principal.issuer,
                    &request.purpose,
                    &request.workflow_definition_id,
                    &request.workflow_definition_version_id,
                    &request.workflow_version,
                    &request.input_schema,
                    &request.output_schema,
                    &(request.execution_timeout_seconds as i32),
                    &(request.inline_result_max_bytes as i32),
                    &artifact_behavior,
                    &json_labels(&request.labels),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!("failed to update workflow endpoint: {error}"))
            })?;
        row.as_ref().map(row_to_endpoint).transpose()
    }

    pub(in crate::session_control) async fn set_workflow_endpoint_state_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint_id: Uuid,
        state: WorkflowEndpointState,
    ) -> Result<Option<StoredWorkflowEndpoint>, SessionStoreError> {
        let query = format!(
            "UPDATE control_workflow_endpoints SET state = $4, updated_at = NOW() WHERE id = $1 AND owner_subject = $2 AND owner_issuer = $3 RETURNING {ENDPOINT_COLUMNS}"
        );
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                &query,
                &[
                    &endpoint_id,
                    &principal.subject,
                    &principal.issuer,
                    &state.as_str(),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow endpoint state: {error}"
                ))
            })?;
        row.as_ref().map(row_to_endpoint).transpose()
    }

    pub(in crate::session_control) async fn upsert_workflow_endpoint_grant_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint: &StoredWorkflowEndpoint,
        request: PersistWorkflowEndpointGrantRequest,
    ) -> Result<StoredWorkflowEndpointGrant, SessionStoreError> {
        let operations = serde_json::to_value(&request.operations).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "failed to encode workflow endpoint grant operations: {error}"
            ))
        })?;
        let query = format!(
            r#"
            INSERT INTO control_workflow_endpoint_grants (
                id, endpoint_id, project_id, service_principal_id,
                operations, created_at, updated_at
            )
            SELECT $1, endpoint.id, endpoint.project_id, $5, $6::jsonb, $7, $7
            FROM control_workflow_endpoints endpoint
            WHERE endpoint.id = $2 AND endpoint.owner_subject = $3 AND endpoint.owner_issuer = $4
            ON CONFLICT (endpoint_id, service_principal_id)
            DO UPDATE SET operations = EXCLUDED.operations, updated_at = EXCLUDED.updated_at
            RETURNING {GRANT_COLUMNS}
            "#
        );
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                &query,
                &[
                    &Uuid::now_v7(),
                    &endpoint.id,
                    &principal.subject,
                    &principal.issuer,
                    &request.service_principal_id,
                    &operations,
                    &Utc::now(),
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to upsert workflow endpoint grant: {error}"
                ))
            })?
            .ok_or_else(|| {
                SessionStoreError::NotFound(format!(
                    "workflow endpoint {} not found",
                    endpoint.endpoint_key
                ))
            })?;
        row_to_grant(&row)
    }

    pub(in crate::session_control) async fn list_workflow_endpoint_grants_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint: &StoredWorkflowEndpoint,
    ) -> Result<Vec<StoredWorkflowEndpointGrant>, SessionStoreError> {
        let query = r#"
            SELECT endpoint_grant.*
            FROM control_workflow_endpoint_grants endpoint_grant
            JOIN control_workflow_endpoints endpoint ON endpoint.id = endpoint_grant.endpoint_id
            WHERE endpoint_grant.endpoint_id = $1 AND endpoint.owner_subject = $2 AND endpoint.owner_issuer = $3
            ORDER BY endpoint_grant.created_at
            "#;
        let rows = self
            .db
            .client()
            .await?
            .query(
                query,
                &[&endpoint.id, &principal.subject, &principal.issuer],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to list workflow endpoint grants: {error}"
                ))
            })?;
        rows.iter().map(row_to_grant).collect()
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_grant(
        &self,
        endpoint_id: Uuid,
        service_principal_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpointGrant>, SessionStoreError> {
        let query = format!(
            "SELECT {GRANT_COLUMNS} FROM control_workflow_endpoint_grants WHERE endpoint_id = $1 AND service_principal_id = $2"
        );
        let row = self
            .db
            .client()
            .await?
            .query_opt(&query, &[&endpoint_id, &service_principal_id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to get workflow endpoint grant: {error}"
                ))
            })?;
        row.as_ref().map(row_to_grant).transpose()
    }

    pub(in crate::session_control) async fn delete_workflow_endpoint_grant_for_owner(
        &self,
        principal: &AuthenticatedPrincipal,
        endpoint: &StoredWorkflowEndpoint,
        grant_id: Uuid,
    ) -> Result<bool, SessionStoreError> {
        let count = self
            .db
            .client()
            .await?
            .execute(
                r#"
                DELETE FROM control_workflow_endpoint_grants endpoint_grant
                USING control_workflow_endpoints endpoint
                WHERE endpoint_grant.id = $1 AND endpoint_grant.endpoint_id = $2
                  AND endpoint.id = endpoint_grant.endpoint_id
                  AND endpoint.owner_subject = $3 AND endpoint.owner_issuer = $4
                "#,
                &[
                    &grant_id,
                    &endpoint.id,
                    &principal.subject,
                    &principal.issuer,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to delete workflow endpoint grant: {error}"
                ))
            })?;
        Ok(count > 0)
    }

    pub(in crate::session_control) async fn reserve_workflow_endpoint_invocation(
        &self,
        request: ReserveWorkflowEndpointInvocationRequest,
    ) -> Result<ReserveWorkflowEndpointInvocationResult, SessionStoreError> {
        let now = Utc::now();
        let query = format!(
            r#"
            INSERT INTO control_workflow_endpoint_invocations (
                id, endpoint_id, caller_service_principal_id, idempotency_key,
                request_fingerprint, run_id, outcome, side_effect_state, created_at, updated_at
            )
            VALUES ($1, $2, $3, $4, $5, NULL, NULL, 'none', $6, $6)
            ON CONFLICT (endpoint_id, caller_service_principal_id, idempotency_key) DO NOTHING
            RETURNING {INVOCATION_COLUMNS}
            "#
        );
        let client = self.db.client().await?;
        let inserted = client
            .query_opt(
                &query,
                &[
                    &Uuid::now_v7(),
                    &request.endpoint_id,
                    &request.caller_service_principal_id,
                    &request.idempotency_key,
                    &request.request_fingerprint,
                    &now,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to reserve workflow endpoint invocation: {error}"
                ))
            })?;
        if let Some(row) = inserted {
            return Ok(ReserveWorkflowEndpointInvocationResult {
                invocation: row_to_invocation(&row)?,
                created: true,
            });
        }
        let select = format!(
            "SELECT {INVOCATION_COLUMNS} FROM control_workflow_endpoint_invocations WHERE endpoint_id = $1 AND caller_service_principal_id = $2 AND idempotency_key = $3"
        );
        let row = client
            .query_one(
                &select,
                &[
                    &request.endpoint_id,
                    &request.caller_service_principal_id,
                    &request.idempotency_key,
                ],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to load reserved workflow endpoint invocation: {error}"
                ))
            })?;
        let invocation = row_to_invocation(&row)?;
        if invocation.request_fingerprint != request.request_fingerprint {
            return Err(SessionStoreError::Conflict(
                "workflow endpoint idempotency key is already bound to a different request"
                    .to_string(),
            ));
        }
        Ok(ReserveWorkflowEndpointInvocationResult {
            invocation,
            created: false,
        })
    }

    pub(in crate::session_control) async fn get_workflow_endpoint_invocation(
        &self,
        endpoint_id: Uuid,
        invocation_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpointInvocation>, SessionStoreError> {
        let query = format!(
            "SELECT {INVOCATION_COLUMNS} FROM control_workflow_endpoint_invocations WHERE id = $1 AND endpoint_id = $2"
        );
        let row = self
            .db
            .client()
            .await?
            .query_opt(&query, &[&invocation_id, &endpoint_id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to get workflow endpoint invocation: {error}"
                ))
            })?;
        row.as_ref().map(row_to_invocation).transpose()
    }

    pub(in crate::session_control) async fn link_workflow_endpoint_invocation_run(
        &self,
        invocation_id: Uuid,
        run_id: Uuid,
    ) -> Result<Option<StoredWorkflowEndpointInvocation>, SessionStoreError> {
        let query = format!(
            "UPDATE control_workflow_endpoint_invocations SET run_id = $2, updated_at = NOW() WHERE id = $1 AND (run_id IS NULL OR run_id = $2) RETURNING {INVOCATION_COLUMNS}"
        );
        let row = self
            .db
            .client()
            .await?
            .query_opt(&query, &[&invocation_id, &run_id])
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to link workflow endpoint invocation run: {error}"
                ))
            })?;
        row.as_ref().map(row_to_invocation).transpose()
    }

    pub(in crate::session_control) async fn fail_workflow_endpoint_invocation(
        &self,
        invocation_id: Uuid,
        outcome: WorkflowRunOutcome,
        side_effect_state: WorkflowSideEffectState,
    ) -> Result<Option<StoredWorkflowEndpointInvocation>, SessionStoreError> {
        let outcome = serde_json::to_value(outcome).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "failed to encode workflow endpoint invocation outcome: {error}"
            ))
        })?;
        let query = format!(
            "UPDATE control_workflow_endpoint_invocations SET outcome = $2::jsonb, side_effect_state = $3, updated_at = NOW() WHERE id = $1 RETURNING {INVOCATION_COLUMNS}"
        );
        let row = self
            .db
            .client()
            .await?
            .query_opt(
                &query,
                &[&invocation_id, &outcome, &side_effect_state.as_str()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to fail workflow endpoint invocation: {error}"
                ))
            })?;
        row.as_ref().map(row_to_invocation).transpose()
    }

    pub(in crate::session_control) async fn update_workflow_run_endpoint_evidence(
        &self,
        run_id: Uuid,
        evidence: WorkflowEndpointRunEvidence,
    ) -> Result<Option<StoredWorkflowRun>, SessionStoreError> {
        let outcome = serde_json::to_value(evidence.outcome).map_err(|error| {
            SessionStoreError::InvalidRequest(format!(
                "failed to encode workflow endpoint run outcome: {error}"
            ))
        })?;
        self.db
            .client()
            .await?
            .execute(
                r#"
                UPDATE control_workflow_runs
                SET endpoint_outcome = $2::jsonb, endpoint_side_effect_state = $3, updated_at = NOW()
                WHERE id = $1 AND endpoint_id IS NOT NULL
                "#,
                &[&run_id, &outcome, &evidence.side_effect_state.as_str()],
            )
            .await
            .map_err(|error| {
                SessionStoreError::Backend(format!(
                    "failed to update workflow endpoint run evidence: {error}"
                ))
            })?;
        self.get_workflow_run_by_id(run_id).await
    }
}

fn row_to_endpoint(row: &Row) -> Result<StoredWorkflowEndpoint, SessionStoreError> {
    let state = row
        .get::<_, String>("state")
        .parse::<WorkflowEndpointState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    let artifact_behavior =
        serde_json::from_value(row.get("artifact_behavior")).map_err(|error| {
            SessionStoreError::Backend(format!(
                "workflow endpoint artifact_behavior must be valid json: {error}"
            ))
        })?;
    let labels = serde_json::from_value(row.get("labels")).map_err(|error| {
        SessionStoreError::Backend(format!(
            "workflow endpoint labels must be valid json: {error}"
        ))
    })?;
    let execution_timeout_seconds: i32 = row.get("execution_timeout_seconds");
    let inline_result_max_bytes: i32 = row.get("inline_result_max_bytes");
    Ok(StoredWorkflowEndpoint {
        id: row.get("id"),
        owner_subject: row.get("owner_subject"),
        owner_issuer: row.get("owner_issuer"),
        project_id: row.get("project_id"),
        endpoint_key: row.get("endpoint_key"),
        purpose: row.get("purpose"),
        workflow_definition_id: row.get("workflow_definition_id"),
        workflow_definition_version_id: row.get("workflow_definition_version_id"),
        workflow_version: row.get("workflow_version"),
        input_schema: row.get("input_schema"),
        output_schema: row.get("output_schema"),
        execution_timeout_seconds: execution_timeout_seconds as u32,
        inline_result_max_bytes: inline_result_max_bytes as u32,
        artifact_behavior,
        labels,
        state,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn row_to_grant(row: &Row) -> Result<StoredWorkflowEndpointGrant, SessionStoreError> {
    let operations = serde_json::from_value(row.get("operations")).map_err(|error| {
        SessionStoreError::Backend(format!(
            "workflow endpoint grant operations must be valid json: {error}"
        ))
    })?;
    Ok(StoredWorkflowEndpointGrant {
        id: row.get("id"),
        endpoint_id: row.get("endpoint_id"),
        project_id: row.get("project_id"),
        service_principal_id: row.get("service_principal_id"),
        operations,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

fn row_to_invocation(row: &Row) -> Result<StoredWorkflowEndpointInvocation, SessionStoreError> {
    let outcome = row
        .get::<_, Option<Value>>("outcome")
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| {
            SessionStoreError::Backend(format!(
                "workflow endpoint invocation outcome must be valid json: {error}"
            ))
        })?;
    let side_effect_state = row
        .get::<_, String>("side_effect_state")
        .parse::<WorkflowSideEffectState>()
        .map_err(|error| SessionStoreError::Backend(error.to_string()))?;
    Ok(StoredWorkflowEndpointInvocation {
        id: row.get("id"),
        endpoint_id: row.get("endpoint_id"),
        caller_service_principal_id: row.get("caller_service_principal_id"),
        idempotency_key: row.get("idempotency_key"),
        request_fingerprint: row.get("request_fingerprint"),
        run_id: row.get("run_id"),
        outcome,
        side_effect_state,
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}
