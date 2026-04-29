use super::*;

#[tokio::test]
async fn in_memory_store_reconciles_workflow_run_from_terminal_task_state() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let fixture = create_workflow_fixture(&store, &owner, "Workflow", "Workflow Task").await;
    let run = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: fixture.workflow.id,
                workflow_definition_version_id: fixture.version.id,
                workflow_version: fixture.version.version.clone(),
                session_id: fixture.session.id,
                automation_task_id: fixture.task.id,
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
        .cancel_automation_task_for_owner(&owner, fixture.task.id)
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
    let fixture = create_workflow_fixture(&store, &owner, "Queued Workflow", "Queued Task").await;
    let run = store
        .create_workflow_run(
            &owner,
            PersistWorkflowRunRequest {
                workflow_definition_id: fixture.workflow.id,
                workflow_definition_version_id: fixture.version.id,
                workflow_version: fixture.version.version.clone(),
                session_id: fixture.session.id,
                automation_task_id: fixture.task.id,
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
            fixture.task.id,
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
        .cancel_automation_task_for_owner(&owner, fixture.task.id)
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
