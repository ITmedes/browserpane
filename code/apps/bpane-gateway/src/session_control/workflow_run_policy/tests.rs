use super::*;

fn sample_task(state: AutomationTaskState) -> StoredAutomationTask {
    let now = Utc::now();
    StoredAutomationTask {
        id: Uuid::now_v7(),
        display_name: Some("task".to_string()),
        executor: "playwright".to_string(),
        state,
        session_id: Uuid::now_v7(),
        session_source: AutomationTaskSessionSource::CreatedSession,
        input: None,
        output: None,
        error: None,
        artifact_refs: Vec::new(),
        labels: HashMap::new(),
        cancel_requested_at: None,
        started_at: None,
        completed_at: None,
        created_at: now,
        updated_at: now,
    }
}

fn sample_run(state: WorkflowRunState) -> StoredWorkflowRun {
    let now = Utc::now();
    StoredWorkflowRun {
        id: Uuid::now_v7(),
        owner_subject: "subject".to_string(),
        owner_issuer: "issuer".to_string(),
        workflow_definition_id: Uuid::now_v7(),
        workflow_definition_version_id: Uuid::now_v7(),
        workflow_version: "v1".to_string(),
        session_id: Uuid::now_v7(),
        automation_task_id: Uuid::now_v7(),
        source_system: None,
        source_reference: None,
        client_request_id: None,
        create_request_fingerprint: None,
        source_snapshot: None,
        extensions: Vec::new(),
        credential_bindings: Vec::new(),
        workspace_inputs: Vec::new(),
        produced_files: Vec::new(),
        state,
        input: None,
        output: None,
        error: None,
        artifact_refs: Vec::new(),
        labels: HashMap::new(),
        started_at: None,
        completed_at: None,
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn transition_plan_sets_started_at_and_default_messages() {
    let task = sample_task(AutomationTaskState::Pending);
    let now = Utc::now();
    let request = WorkflowRunTransitionRequest {
        state: WorkflowRunState::Running,
        output: None,
        error: None,
        artifact_refs: vec!["artifact://run".to_string()],
        message: None,
        data: Some(serde_json::json!({ "step": 1 })),
    };

    let plan = plan_workflow_run_transition(&task, &request, now).unwrap();

    assert_eq!(plan.task_state, AutomationTaskState::Running);
    assert_eq!(plan.task_started_at, Some(now));
    assert_eq!(plan.task_completed_at, None);
    assert_eq!(plan.task_event_type, "automation_task.running");
    assert_eq!(plan.task_event_message, "automation task entered running state");
    assert_eq!(plan.run_state, WorkflowRunState::Running);
    assert_eq!(plan.run_event_type, "workflow_run.running");
    assert_eq!(plan.run_event_message, "workflow run entered running state");
    assert_eq!(plan.run_artifact_refs, vec!["artifact://run".to_string()]);
    assert_eq!(plan.run_event_data, Some(serde_json::json!({ "step": 1 })));
}

#[test]
fn transition_plan_rejects_invalid_task_transition() {
    let task = sample_task(AutomationTaskState::Pending);
    let request = WorkflowRunTransitionRequest {
        state: WorkflowRunState::AwaitingInput,
        output: None,
        error: None,
        artifact_refs: Vec::new(),
        message: None,
        data: None,
    };

    let error = plan_workflow_run_transition(&task, &request, Utc::now()).unwrap_err();
    assert_eq!(
        error,
        WorkflowRunTransitionPolicyError::InvalidTaskTransition {
            task_id: task.id,
            current: AutomationTaskState::Pending,
            next: AutomationTaskState::AwaitingInput,
        }
    );
}

#[test]
fn transition_plan_rejects_terminal_task() {
    let task = sample_task(AutomationTaskState::Succeeded);
    let request = WorkflowRunTransitionRequest {
        state: WorkflowRunState::Failed,
        output: None,
        error: Some("boom".to_string()),
        artifact_refs: Vec::new(),
        message: Some("failed".to_string()),
        data: None,
    };

    let error = plan_workflow_run_transition(&task, &request, Utc::now()).unwrap_err();
    assert_eq!(
        error,
        WorkflowRunTransitionPolicyError::TaskAlreadyTerminal { task_id: task.id }
    );
}

#[test]
fn reconciliation_plan_returns_update_for_changed_terminal_task() {
    let mut run = sample_run(WorkflowRunState::Running);
    let mut task = sample_task(AutomationTaskState::Succeeded);
    let now = Utc::now();

    task.output = Some(serde_json::json!({ "result": "ok" }));
    task.artifact_refs = vec!["artifact://done".to_string()];
    task.started_at = Some(now - ChronoDuration::minutes(1));
    task.completed_at = Some(now);
    run.automation_task_id = task.id;

    let (decision, plan) = plan_workflow_run_reconciliation(&run, &task, now);

    assert_eq!(decision, WorkflowRunReconciliationDecision::Update);
    let plan = plan.expect("expected reconciliation update");
    assert_eq!(plan.run_state, WorkflowRunState::Succeeded);
    assert_eq!(plan.run_output, task.output);
    assert_eq!(plan.run_artifact_refs, task.artifact_refs);
    assert_eq!(plan.run_started_at, task.started_at);
    assert_eq!(plan.run_completed_at, task.completed_at);
    assert_eq!(plan.run_event_type, "workflow_run.succeeded");
    assert_eq!(
        plan.run_event_message,
        "workflow run reconciled from terminal automation task state"
    );
}

#[test]
fn reconciliation_plan_returns_unchanged_for_matching_terminal_state() {
    let now = Utc::now();
    let mut run = sample_run(WorkflowRunState::Succeeded);
    let mut task = sample_task(AutomationTaskState::Succeeded);

    let output = Some(serde_json::json!({ "result": "ok" }));
    let artifacts = vec!["artifact://done".to_string()];
    let started_at = Some(now - ChronoDuration::minutes(1));
    let completed_at = Some(now);

    run.output = output.clone();
    run.artifact_refs = artifacts.clone();
    run.started_at = started_at;
    run.completed_at = completed_at;
    task.output = output;
    task.artifact_refs = artifacts;
    task.started_at = started_at;
    task.completed_at = completed_at;

    let (decision, plan) = plan_workflow_run_reconciliation(&run, &task, now);

    assert_eq!(decision, WorkflowRunReconciliationDecision::Unchanged);
    assert!(plan.is_none());
}

#[test]
fn reconciliation_plan_returns_not_terminal_for_live_task() {
    let run = sample_run(WorkflowRunState::Running);
    let task = sample_task(AutomationTaskState::Running);

    let (decision, plan) = plan_workflow_run_reconciliation(&run, &task, Utc::now());

    assert_eq!(decision, WorkflowRunReconciliationDecision::NotTerminal);
    assert!(plan.is_none());
}
