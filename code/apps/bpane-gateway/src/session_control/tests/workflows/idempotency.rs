use super::*;

#[tokio::test]
async fn in_memory_store_deduplicates_workflow_runs_by_client_request_id() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let fixture = create_workflow_fixture(&store, &owner, "Workflow", "Workflow Task").await;

    let first = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: fixture.workflow.id,
                workflow_definition_version_id: fixture.version.id,
                workflow_version: fixture.version.version.clone(),
                session_id: fixture.session.id,
                automation_task_id: fixture.task.id,
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
                workflow_definition_id: fixture.workflow.id,
                workflow_definition_version_id: fixture.version.id,
                workflow_version: fixture.version.version.clone(),
                session_id: fixture.session.id,
                automation_task_id: fixture.task.id,
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
async fn in_memory_store_rejects_conflicting_workflow_run_client_request_id() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let fixture = create_workflow_fixture(&store, &owner, "Workflow", "Workflow Task").await;

    let created = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: fixture.workflow.id,
                workflow_definition_version_id: fixture.version.id,
                workflow_version: fixture.version.version.clone(),
                session_id: fixture.session.id,
                automation_task_id: fixture.task.id,
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
                workflow_definition_id: fixture.workflow.id,
                workflow_definition_version_id: fixture.version.id,
                workflow_version: fixture.version.version.clone(),
                session_id: fixture.session.id,
                automation_task_id: fixture.task.id,
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
