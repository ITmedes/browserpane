use anyhow::{ensure, Context};

use crate::workflow::WorkflowPackageManifest;

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
