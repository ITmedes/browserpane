use super::support::principal;
use super::*;

#[tokio::test]
async fn in_memory_store_deduplicates_workflow_runs_by_client_request_id() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let workflow = store
        .create_workflow_definition(
            &owner,
            PersistWorkflowDefinitionRequest {
                name: "Workflow".to_string(),
                description: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let version = store
        .create_workflow_definition_version(
            &owner,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: workflow.id,
                version: "v1".to_string(),
                executor: "playwright".to_string(),
                entrypoint: "workflows/run.mjs".to_string(),
                source: None,
                input_schema: None,
                output_schema: None,
                default_session: None,
                allowed_credential_binding_ids: Vec::new(),
                allowed_extension_ids: Vec::new(),
                allowed_file_workspace_ids: Vec::new(),
            },
        )
        .await
        .unwrap();

    let session_one = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let task_one = store
        .create_automation_task(
            &owner,
            PersistAutomationTaskRequest {
                display_name: Some("Workflow Task".to_string()),
                executor: "playwright".to_string(),
                session_id: session_one.id,
                session_source: AutomationTaskSessionSource::CreatedSession,
                input: Some(serde_json::json!({ "customer_id": "cust-42" })),
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();

    let first = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: workflow.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                session_id: session_one.id,
                automation_task_id: task_one.id,
                source_system: Some("camunda-prod".to_string()),
                source_reference: Some("task-1".to_string()),
                client_request_id: Some("job-123-attempt-1".to_string()),
                create_request_fingerprint: Some("fingerprint-a".to_string()),
                source_snapshot: None,
                extensions: Vec::new(),
                credential_bindings: Vec::new(),
                workspace_inputs: Vec::new(),
                input: Some(serde_json::json!({ "customer_id": "cust-42" })),
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    assert!(first.created);

    let second = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: workflow.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                session_id: session_one.id,
                automation_task_id: task_one.id,
                source_system: Some("camunda-prod".to_string()),
                source_reference: Some("task-1".to_string()),
                client_request_id: Some("job-123-attempt-1".to_string()),
                create_request_fingerprint: Some("fingerprint-a".to_string()),
                source_snapshot: None,
                extensions: Vec::new(),
                credential_bindings: Vec::new(),
                workspace_inputs: Vec::new(),
                input: Some(serde_json::json!({ "customer_id": "cust-42" })),
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();

    assert!(!second.created);
    assert_eq!(second.run.id, first.run.id);
    assert_eq!(second.run.session_id, first.run.session_id);
    assert_eq!(second.run.automation_task_id, first.run.automation_task_id);
    assert_eq!(
        store
            .find_workflow_run_by_client_request_id_for_owner(&owner, "job-123-attempt-1")
            .await
            .unwrap()
            .unwrap()
            .id,
        first.run.id
    );
}

#[tokio::test]
async fn in_memory_store_reconciles_workflow_run_from_terminal_task_state() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let session = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let task = store
        .create_automation_task(
            &owner,
            PersistAutomationTaskRequest {
                display_name: Some("Workflow Task".to_string()),
                executor: "playwright".to_string(),
                session_id: session.id,
                session_source: AutomationTaskSessionSource::CreatedSession,
                input: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let workflow = store
        .create_workflow_definition(
            &owner,
            PersistWorkflowDefinitionRequest {
                name: "Workflow".to_string(),
                description: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let version = store
        .create_workflow_definition_version(
            &owner,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: workflow.id,
                version: "v1".to_string(),
                executor: "playwright".to_string(),
                entrypoint: "workflows/run.mjs".to_string(),
                source: None,
                input_schema: None,
                output_schema: None,
                default_session: None,
                allowed_credential_binding_ids: Vec::new(),
                allowed_extension_ids: Vec::new(),
                allowed_file_workspace_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    let run = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: workflow.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                session_id: session.id,
                automation_task_id: task.id,
                source_system: None,
                source_reference: None,
                client_request_id: None,
                create_request_fingerprint: None,
                source_snapshot: None,
                extensions: Vec::new(),
                credential_bindings: Vec::new(),
                workspace_inputs: Vec::new(),
                input: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap()
        .run;

    store
        .cancel_automation_task_for_owner(&owner, task.id)
        .await
        .unwrap()
        .unwrap();

    let reconciled = store
        .reconcile_workflow_run_from_task(run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reconciled.state, WorkflowRunState::Cancelled);
    assert!(reconciled.completed_at.is_some());
}

#[tokio::test]
async fn in_memory_store_cancels_queued_automation_task_and_workflow_run() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let session = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let task = store
        .create_automation_task(
            &owner,
            PersistAutomationTaskRequest {
                display_name: Some("Queued Task".to_string()),
                executor: "playwright".to_string(),
                session_id: session.id,
                session_source: AutomationTaskSessionSource::CreatedSession,
                input: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let workflow = store
        .create_workflow_definition(
            &owner,
            PersistWorkflowDefinitionRequest {
                name: "Queued Workflow".to_string(),
                description: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let version = store
        .create_workflow_definition_version(
            &owner,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: workflow.id,
                version: "v1".to_string(),
                executor: "playwright".to_string(),
                entrypoint: "workflows/run.mjs".to_string(),
                source: None,
                input_schema: None,
                output_schema: None,
                default_session: None,
                allowed_credential_binding_ids: Vec::new(),
                allowed_extension_ids: Vec::new(),
                allowed_file_workspace_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    let run = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: workflow.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                session_id: session.id,
                automation_task_id: task.id,
                source_system: None,
                source_reference: None,
                client_request_id: None,
                create_request_fingerprint: None,
                source_snapshot: None,
                extensions: Vec::new(),
                credential_bindings: Vec::new(),
                workspace_inputs: Vec::new(),
                input: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap()
        .run;

    let queued = store
        .transition_automation_task(
            task.id,
            AutomationTaskTransitionRequest {
                state: AutomationTaskState::Queued,
                output: None,
                error: None,
                artifact_refs: Vec::new(),
                event_type: "automation_task.queued".to_string(),
                event_message: "task queued while waiting for worker capacity".to_string(),
                event_data: None,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(queued.state, AutomationTaskState::Queued);

    let cancelled = store
        .cancel_automation_task_for_owner(&owner, task.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cancelled.state, AutomationTaskState::Cancelled);
    assert!(cancelled.cancel_requested_at.is_some());
    assert!(cancelled.completed_at.is_some());

    let current_run = store.get_workflow_run_by_id(run.id).await.unwrap().unwrap();
    assert_eq!(current_run.state, WorkflowRunState::Cancelled);
    assert!(current_run.completed_at.is_some());

    let events = store
        .list_workflow_run_events_for_owner(&owner, run.id)
        .await
        .unwrap();
    assert!(events
        .iter()
        .any(|event| event.event_type == "workflow_run.cancelled"));
}

#[tokio::test]
async fn in_memory_store_rejects_conflicting_workflow_run_client_request_id() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let session = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let task = store
        .create_automation_task(
            &owner,
            PersistAutomationTaskRequest {
                display_name: Some("Workflow Task".to_string()),
                executor: "playwright".to_string(),
                session_id: session.id,
                session_source: AutomationTaskSessionSource::CreatedSession,
                input: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let workflow = store
        .create_workflow_definition(
            &owner,
            PersistWorkflowDefinitionRequest {
                name: "Workflow".to_string(),
                description: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let version = store
        .create_workflow_definition_version(
            &owner,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: workflow.id,
                version: "v1".to_string(),
                executor: "playwright".to_string(),
                entrypoint: "workflows/run.mjs".to_string(),
                source: None,
                input_schema: None,
                output_schema: None,
                default_session: None,
                allowed_credential_binding_ids: Vec::new(),
                allowed_extension_ids: Vec::new(),
                allowed_file_workspace_ids: Vec::new(),
            },
        )
        .await
        .unwrap();

    let created = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: workflow.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                session_id: session.id,
                automation_task_id: task.id,
                source_system: Some("camunda-prod".to_string()),
                source_reference: Some("task-1".to_string()),
                client_request_id: Some("job-123-attempt-1".to_string()),
                create_request_fingerprint: Some("fingerprint-a".to_string()),
                source_snapshot: None,
                extensions: Vec::new(),
                credential_bindings: Vec::new(),
                workspace_inputs: Vec::new(),
                input: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    assert!(created.created);

    let error = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: workflow.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version.clone(),
                session_id: session.id,
                automation_task_id: task.id,
                source_system: Some("camunda-prod".to_string()),
                source_reference: Some("task-2".to_string()),
                client_request_id: Some("job-123-attempt-1".to_string()),
                create_request_fingerprint: Some("fingerprint-b".to_string()),
                source_snapshot: None,
                extensions: Vec::new(),
                credential_bindings: Vec::new(),
                workspace_inputs: Vec::new(),
                input: Some(serde_json::json!({ "customer_id": "cust-77" })),
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap_err();
    assert!(
        matches!(error, SessionStoreError::Conflict(message) if message.contains("client_request_id"))
    );
}
