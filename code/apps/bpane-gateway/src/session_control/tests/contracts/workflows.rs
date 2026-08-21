use anyhow::{ensure, Context};

use crate::workflow::WorkflowPackageManifest;
use crate::workflow_endpoints::{
    WorkflowEndpointArtifactBehavior, WorkflowEndpointGrantOperation, WorkflowEndpointRunContext,
    WorkflowOutcomeCategory,
};

use super::*;

pub(super) async fn run_workflow_contracts(store: &SessionStore) -> anyhow::Result<()> {
    let suffix = Uuid::now_v7().simple().to_string();
    let owner = principal(&format!("workflow-owner-{suffix}"));
    let other_owner = principal(&format!("workflow-other-{suffix}"));
    let session = store
        .create_session(
            &owner,
            CreateSessionRequest::default(),
            SessionOwnerMode::Collaborative,
        )
        .await
        .context("create workflow contract session")?;
    let task = store
        .create_automation_task(
            &owner,
            PersistAutomationTaskRequest {
                display_name: Some("Contract workflow task".to_string()),
                executor: "playwright".to_string(),
                session_id: session.id,
                session_source: AutomationTaskSessionSource::CreatedSession,
                input: Some(serde_json::json!({ "contract": true })),
                labels: HashMap::from([("contract".to_string(), "workflow".to_string())]),
            },
        )
        .await
        .context("create workflow contract automation task")?;
    ensure!(
        store
            .get_automation_task_for_owner(&other_owner, task.id)
            .await
            .context("read workflow task as another owner")?
            .is_none(),
        "workflow task was visible to another owner"
    );

    let definition = store
        .create_workflow_definition(
            &owner,
            PersistWorkflowDefinitionRequest {
                name: format!("contract-workflow-{suffix}"),
                description: Some("contract workflow".to_string()),
                labels: HashMap::from([("contract".to_string(), "workflow".to_string())]),
            },
        )
        .await
        .context("create contract workflow definition")?;
    ensure!(
        store
            .get_workflow_definition_for_owner(&other_owner, definition.id)
            .await
            .context("read workflow definition as another owner")?
            .is_none(),
        "workflow definition was visible to another owner"
    );
    let version = store
        .create_workflow_definition_version(
            &owner,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: definition.id,
                version: "v1".to_string(),
                executor: "playwright".to_string(),
                entrypoint: "workflows/run.mjs".to_string(),
                source: None,
                input_schema: Some(serde_json::json!({ "type": "object" })),
                output_schema: Some(serde_json::json!({ "type": "object" })),
                package: None,
                default_session: None,
                allowed_credential_binding_ids: Vec::new(),
                allowed_extension_ids: Vec::new(),
                allowed_file_workspace_ids: Vec::new(),
            },
        )
        .await
        .context("create contract workflow version")?;
    ensure!(
        store
            .get_workflow_definition_version_for_owner(&other_owner, definition.id, "v1")
            .await
            .context("read workflow version as another owner")?
            .is_none(),
        "workflow version was visible to another owner"
    );
    let supported_package: WorkflowPackageManifest = serde_json::from_value(serde_json::json!({
        "package_id": "contract.workflow.v2",
        "format_version": "browserpane.workflow-package/v1",
        "runtime": {
            "language": "typescript",
            "browserpane_api_version": "v1",
            "node_major_version": 22,
            "playwright_major_version": 1,
            "playwright_minor_version": 59
        },
        "requirements": {
            "default_session": {
                "project_id": null,
                "browser_context": { "mode": "fresh", "context_id": null },
                "network_identity": { "egress_profile_id": null },
                "capabilities": {
                    "browser_input": true,
                    "clipboard": false,
                    "audio": false,
                    "microphone": false,
                    "camera": false,
                    "file_transfer": false,
                    "resize": true
                },
                "recording": { "mode": "disabled", "format": "webm", "retention_sec": null },
                "extension_ids": []
            },
            "allowed_credential_binding_ids": [],
            "allowed_extension_ids": [],
            "allowed_file_workspace_ids": []
        },
        "execution": {
            "timeout_ms": 60000,
            "assertions": ["contract-output"],
            "safe_cancellation_points": ["before-submit"],
            "side_effect_checkpoints": ["after-submit"]
        },
        "publication": {
            "reviewer": "session-store-contract",
            "reviewed_at": "2026-08-20T12:00:00Z",
            "decision": "approved",
            "fresh_context_replay": true,
            "scenarios": [
                { "kind": "happy_path", "result": "passed" },
                { "kind": "validation", "result": "passed" },
                { "kind": "missing_element", "result": "passed" },
                { "kind": "authentication_challenge", "result": "passed" },
                { "kind": "portal_failure", "result": "passed" },
                { "kind": "runtime_failure", "result": "passed" },
                { "kind": "cancellation", "result": "passed" },
                { "kind": "ambiguous_post_side_effect", "result": "passed" }
            ]
        }
    }))?;
    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object"
    });
    let supported_version = store
        .create_workflow_definition_version(
            &owner,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: definition.id,
                version: "v2".to_string(),
                executor: "playwright".to_string(),
                entrypoint: "workflows/run.ts".to_string(),
                source: Some(serde_json::from_value(serde_json::json!({
                    "kind": "git",
                    "repository_url": "https://example.test/workflow.git",
                    "ref": "main",
                    "resolved_commit": "a".repeat(40),
                    "root_path": "workflows"
                }))?),
                input_schema: Some(schema.clone()),
                output_schema: Some(schema),
                package: Some(supported_package.clone()),
                default_session: Some(supported_package.requirements.default_session.clone()),
                allowed_credential_binding_ids: Vec::new(),
                allowed_extension_ids: Vec::new(),
                allowed_file_workspace_ids: Vec::new(),
            },
        )
        .await
        .context("create supported contract workflow version")?;
    ensure!(
        supported_version.package.as_ref() == Some(&supported_package),
        "supported workflow package did not round-trip through the session store"
    );
    workflow_endpoint_contract(
        store,
        &owner,
        &other_owner,
        &definition,
        &supported_version,
        &suffix,
    )
    .await?;

    let client_request_id = format!("contract-request-{suffix}");
    let fingerprint = format!("contract-fingerprint-{suffix}");
    let request = || PersistWorkflowRunRequest {
        workflow_definition_id: definition.id,
        workflow_definition_version_id: version.id,
        workflow_version: version.version.clone(),
        project_id: None,
        session_id: session.id,
        automation_task_id: task.id,
        source_system: Some("contract-suite".to_string()),
        source_reference: Some(format!("source-{suffix}")),
        client_request_id: Some(client_request_id.clone()),
        create_request_fingerprint: Some(fingerprint.clone()),
        endpoint: None,
        source_snapshot: None,
        extensions: Vec::new(),
        credential_bindings: Vec::new(),
        workspace_inputs: Vec::new(),
        input: Some(serde_json::json!({ "case": suffix })),
        labels: HashMap::from([("contract".to_string(), "run".to_string())]),
    };
    let first = store
        .create_workflow_run(&owner, request())
        .await
        .context("create contract workflow run")?;
    ensure!(first.created, "first workflow run was not marked created");
    ensure!(
        store
            .get_workflow_run_for_owner(&other_owner, first.run.id)
            .await
            .context("read workflow run as another owner")?
            .is_none(),
        "workflow run was visible to another owner"
    );
    ensure!(
        store
            .list_workflow_runs_for_owner(&owner)
            .await
            .context("list owner workflow runs")?
            .iter()
            .any(|run| run.id == first.run.id),
        "workflow run was missing from the owner catalog"
    );

    let duplicate = store
        .create_workflow_run(&owner, request())
        .await
        .context("deduplicate contract workflow run")?;
    ensure!(
        !duplicate.created && duplicate.run.id == first.run.id,
        "workflow run idempotency contract diverged"
    );
    let conflict_error = expected_store_error(
        store
            .create_workflow_run(
                &owner,
                PersistWorkflowRunRequest {
                    create_request_fingerprint: Some(format!("different-{suffix}")),
                    source_reference: Some(format!("different-source-{suffix}")),
                    ..request()
                },
            )
            .await,
        "conflicting workflow request id should be rejected",
    )?;
    ensure!(
        matches!(conflict_error, SessionStoreError::Conflict(_)),
        "workflow idempotency conflict returned the wrong error class"
    );
    ensure!(
        store
            .find_workflow_run_by_client_request_id_for_owner(&owner, &client_request_id)
            .await
            .context("find workflow run by client request id")?
            .is_some_and(|run| run.id == first.run.id),
        "workflow run lookup by client request id diverged"
    );
    ensure!(
        store
            .find_workflow_run_by_client_request_id_for_owner(&other_owner, &client_request_id)
            .await
            .context("find workflow run by client request id as another owner")?
            .is_none(),
        "workflow client request id leaked across owners"
    );

    ensure!(
        store
            .cancel_automation_task_for_owner(&other_owner, task.id)
            .await
            .context("cancel workflow task as another owner")?
            .is_none(),
        "another owner cancelled the workflow task"
    );
    let cancelled_task = store
        .cancel_automation_task_for_owner(&owner, task.id)
        .await
        .context("cancel contract workflow task")?
        .context("contract workflow task disappeared before cancellation")?;
    ensure!(
        cancelled_task.state == AutomationTaskState::Cancelled
            && cancelled_task.completed_at.is_some(),
        "workflow task cancellation was not persisted"
    );
    let cancelled_run = store
        .reconcile_workflow_run_from_task(first.run.id)
        .await
        .context("reconcile workflow run from cancelled task")?
        .context("contract workflow run disappeared during reconciliation")?;
    ensure!(
        cancelled_run.state == WorkflowRunState::Cancelled && cancelled_run.completed_at.is_some(),
        "workflow run did not reconcile to cancelled"
    );
    ensure!(
        store
            .list_workflow_run_events_for_owner(&other_owner, first.run.id)
            .await
            .context("list workflow events as another owner")?
            .is_empty(),
        "workflow events leaked across owners"
    );
    ensure!(
        store
            .list_workflow_run_events_for_owner(&owner, first.run.id)
            .await
            .context("list owner workflow events")?
            .iter()
            .any(|event| event.event_type == "workflow_run.cancelled"),
        "workflow cancellation event was not persisted"
    );
    Ok(())
}

async fn workflow_endpoint_contract(
    store: &SessionStore,
    owner: &AuthenticatedPrincipal,
    other_owner: &AuthenticatedPrincipal,
    definition: &StoredWorkflowDefinition,
    version: &StoredWorkflowDefinitionVersion,
    suffix: &str,
) -> anyhow::Result<()> {
    let project = store
        .create_project(
            owner,
            PersistProjectRequest {
                name: format!("workflow-endpoint-project-{suffix}"),
                description: Some("workflow endpoint contract project".to_string()),
                labels: HashMap::new(),
                quotas: ProjectQuotas::default(),
                policy: ProjectPolicy::default(),
                state: ProjectState::Active,
            },
        )
        .await
        .context("create workflow endpoint contract project")?;
    let service_principal = store
        .create_service_principal(
            owner,
            PersistServicePrincipalRequest {
                name: "Workflow endpoint contract caller".to_string(),
                description: None,
                client_id: format!("workflow-endpoint-client-{suffix}"),
                issuer: format!("https://workflow-endpoint-{suffix}.example"),
                labels: HashMap::new(),
                scopes: vec![
                    "workflow-endpoints:invoke".to_string(),
                    "workflow-endpoints:read".to_string(),
                    "workflow-endpoints:cancel".to_string(),
                    "workflow-endpoints:artifact.read".to_string(),
                ],
                allowed_project_ids: vec![project.id],
                state: ServicePrincipalState::Active,
            },
        )
        .await
        .context("create workflow endpoint contract service principal")?;
    let input_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["reporting_period"],
        "properties": { "reporting_period": { "type": "string" } },
        "additionalProperties": false
    });
    let output_schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": ["ok"],
        "properties": { "ok": { "type": "boolean" } },
        "additionalProperties": false
    });
    let endpoint_request = || PersistWorkflowEndpointRequest {
        project_id: project.id,
        endpoint_key: "retrieve-supplier-report".to_string(),
        purpose: "Retrieve one approved supplier report".to_string(),
        workflow_definition_id: definition.id,
        workflow_definition_version_id: version.id,
        workflow_version: version.version.clone(),
        input_schema: input_schema.clone(),
        output_schema: output_schema.clone(),
        execution_timeout_seconds: 300,
        inline_result_max_bytes: 65_536,
        artifact_behavior: WorkflowEndpointArtifactBehavior::default(),
        labels: HashMap::from([("contract".to_string(), "endpoint".to_string())]),
    };
    let endpoint = store
        .create_workflow_endpoint(owner, endpoint_request())
        .await
        .context("create workflow endpoint contract resource")?;
    ensure!(
        endpoint.state == WorkflowEndpointState::Draft,
        "new workflow endpoint was not draft"
    );
    ensure!(
        store
            .get_workflow_endpoint_for_owner_project_key(
                other_owner,
                project.id,
                &endpoint.endpoint_key,
            )
            .await
            .context("read workflow endpoint as another owner")?
            .is_none(),
        "workflow endpoint leaked across owners"
    );
    let grant = store
        .upsert_workflow_endpoint_grant_for_owner(
            owner,
            &endpoint,
            PersistWorkflowEndpointGrantRequest {
                service_principal_id: service_principal.id,
                operations: vec![
                    WorkflowEndpointGrantOperation::Invoke,
                    WorkflowEndpointGrantOperation::Read,
                    WorkflowEndpointGrantOperation::Cancel,
                    WorkflowEndpointGrantOperation::ArtifactRead,
                ],
            },
        )
        .await
        .context("create workflow endpoint contract grant")?;
    ensure!(
        store
            .get_workflow_endpoint_grant(endpoint.id, service_principal.id)
            .await
            .context("read workflow endpoint contract grant")?
            .is_some_and(|stored| stored.id == grant.id),
        "workflow endpoint grant did not round trip"
    );
    let endpoint = store
        .set_workflow_endpoint_state_for_owner(owner, endpoint.id, WorkflowEndpointState::Active)
        .await
        .context("activate workflow endpoint contract resource")?
        .context("workflow endpoint disappeared during activation")?;
    let active_update_error = expected_store_error(
        store
            .update_workflow_endpoint_for_owner(owner, endpoint.id, endpoint_request())
            .await,
        "active workflow endpoint update should be rejected",
    )?;
    ensure!(
        matches!(active_update_error, SessionStoreError::Conflict(_)),
        "active workflow endpoint update returned the wrong error class"
    );

    let reservation_request = ReserveWorkflowEndpointInvocationRequest {
        endpoint_id: endpoint.id,
        caller_service_principal_id: service_principal.id,
        idempotency_key: format!("contract-idempotency-{suffix}"),
        request_fingerprint: "a".repeat(64),
    };
    let (left, right) = tokio::join!(
        store.reserve_workflow_endpoint_invocation(reservation_request.clone()),
        store.reserve_workflow_endpoint_invocation(reservation_request.clone())
    );
    let left = left.context("reserve first concurrent workflow endpoint invocation")?;
    let right = right.context("reserve second concurrent workflow endpoint invocation")?;
    ensure!(
        left.invocation.id == right.invocation.id && left.created != right.created,
        "concurrent identical workflow endpoint invocations did not converge on one reservation"
    );
    let invocation = left.invocation;
    let conflict = expected_store_error(
        store
            .reserve_workflow_endpoint_invocation(ReserveWorkflowEndpointInvocationRequest {
                request_fingerprint: "b".repeat(64),
                ..reservation_request
            })
            .await,
        "changed workflow endpoint invocation fingerprint should conflict",
    )?;
    ensure!(
        matches!(conflict, SessionStoreError::Conflict(_)),
        "workflow endpoint idempotency conflict returned the wrong error class"
    );

    let session = store
        .create_session(
            owner,
            CreateSessionRequest {
                project_id: Some(project.id),
                ..CreateSessionRequest::default()
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .context("create workflow endpoint contract session")?;
    let task = store
        .create_automation_task(
            owner,
            PersistAutomationTaskRequest {
                display_name: Some("Workflow endpoint contract task".to_string()),
                executor: version.executor.clone(),
                session_id: session.id,
                session_source: AutomationTaskSessionSource::CreatedSession,
                input: Some(serde_json::json!({ "reporting_period": "2026-Q3" })),
                labels: HashMap::new(),
            },
        )
        .await
        .context("create workflow endpoint contract task")?;
    let run = store
        .create_workflow_run(
            owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: definition.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                project_id: Some(project.id),
                session_id: session.id,
                automation_task_id: task.id,
                source_system: Some("contract-bpm".to_string()),
                source_reference: Some(format!("process-{suffix}")),
                client_request_id: Some(format!("endpoint-run-{suffix}")),
                create_request_fingerprint: Some("c".repeat(64)),
                endpoint: Some(WorkflowEndpointRunContext {
                    endpoint_id: endpoint.id,
                    invocation_id: invocation.id,
                    endpoint_key: endpoint.endpoint_key.clone(),
                    caller_service_principal_id: service_principal.id,
                    idempotency_key: invocation.idempotency_key.clone(),
                    request_fingerprint: invocation.request_fingerprint.clone(),
                    execution_deadline_at: Utc::now() + chrono::Duration::minutes(5),
                }),
                source_snapshot: None,
                extensions: Vec::new(),
                credential_bindings: Vec::new(),
                workspace_inputs: Vec::new(),
                input: Some(serde_json::json!({ "reporting_period": "2026-Q3" })),
                labels: HashMap::new(),
            },
        )
        .await
        .context("create workflow endpoint contract run")?
        .run;
    store
        .link_workflow_endpoint_invocation_run(invocation.id, run.id)
        .await
        .context("link workflow endpoint contract run")?
        .context("workflow endpoint invocation disappeared during run linking")?;
    for (state, event_type) in [
        (AutomationTaskState::Queued, "automation_task.queued"),
        (AutomationTaskState::Running, "automation_task.running"),
    ] {
        store
            .transition_automation_task(
                task.id,
                AutomationTaskTransitionRequest {
                    state,
                    output: None,
                    error: None,
                    artifact_refs: Vec::new(),
                    event_type: event_type.to_string(),
                    event_message: event_type.to_string(),
                    event_data: None,
                },
            )
            .await
            .with_context(|| format!("transition workflow endpoint task to {}", state.as_str()))?
            .context("workflow endpoint task disappeared during transition")?;
    }
    store
        .transition_automation_task(
            task.id,
            AutomationTaskTransitionRequest {
                state: AutomationTaskState::Succeeded,
                output: Some(serde_json::json!({ "ok": "not-a-boolean" })),
                error: None,
                artifact_refs: Vec::new(),
                event_type: "automation_task.succeeded".to_string(),
                event_message: "workflow endpoint contract output".to_string(),
                event_data: None,
            },
        )
        .await
        .context("attempt invalid workflow endpoint success")?
        .context("workflow endpoint task disappeared during invalid success")?;
    let run = store
        .get_workflow_run_by_id(run.id)
        .await
        .context("read workflow endpoint run after invalid output")?
        .context("workflow endpoint run disappeared after invalid output")?;
    ensure!(
        run.state == WorkflowRunState::Failed
            && run.output.is_none()
            && run.outcome.as_ref().is_some_and(|outcome| {
                outcome.category == WorkflowOutcomeCategory::ValidationFailure
            })
            && run.side_effect_state == Some(WorkflowSideEffectState::Uncertain),
        "invalid endpoint output was allowed to succeed or lost terminal evidence"
    );
    Ok(())
}
