use super::*;

fn stored_task(state: AutomationTaskState) -> StoredAutomationTask {
    let now = Utc::now();
    StoredAutomationTask {
        id: Uuid::now_v7(),
        display_name: Some("demo task".to_string()),
        executor: "playwright".to_string(),
        state,
        session_id: Uuid::now_v7(),
        session_source: AutomationTaskSessionSource::ExistingSession,
        input: Some(serde_json::json!({ "step": "login" })),
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

#[test]
fn transition_plan_sets_started_at_and_keeps_event_payload() {
    let task = stored_task(AutomationTaskState::Pending);
    let now = Utc::now();
    let request = AutomationTaskTransitionRequest {
        state: AutomationTaskState::Running,
        output: None,
        error: None,
        artifact_refs: vec!["artifact://trace.zip".to_string()],
        event_type: "automation_task.running".to_string(),
        event_message: "task entered running state".to_string(),
        event_data: Some(serde_json::json!({ "step": "login" })),
    };

    let plan = plan_automation_task_transition(&task, &request, now).unwrap();

    assert_eq!(plan.task_state, AutomationTaskState::Running);
    assert_eq!(plan.task_started_at, Some(now));
    assert_eq!(plan.task_completed_at, None);
    assert_eq!(plan.task_artifact_refs, request.artifact_refs);
    assert_eq!(plan.task_event_type, "automation_task.running");
    assert_eq!(plan.task_event_message, "task entered running state");
    assert_eq!(
        plan.task_event_data,
        Some(serde_json::json!({ "step": "login" }))
    );
}

#[test]
fn transition_plan_rejects_invalid_task_transition() {
    let task = stored_task(AutomationTaskState::Pending);
    let request = AutomationTaskTransitionRequest {
        state: AutomationTaskState::AwaitingInput,
        output: None,
        error: None,
        artifact_refs: Vec::new(),
        event_type: "automation_task.awaiting_input".to_string(),
        event_message: "waiting for operator".to_string(),
        event_data: None,
    };

    let error = plan_automation_task_transition(&task, &request, Utc::now()).unwrap_err();

    assert!(matches!(
        error,
        AutomationTaskTransitionPolicyError::InvalidTaskTransition { .. }
    ));
}

#[test]
fn transition_plan_rejects_terminal_task() {
    let task = stored_task(AutomationTaskState::Succeeded);
    let request = AutomationTaskTransitionRequest {
        state: AutomationTaskState::Running,
        output: None,
        error: None,
        artifact_refs: Vec::new(),
        event_type: "automation_task.running".to_string(),
        event_message: "task re-entered running".to_string(),
        event_data: None,
    };

    let error = plan_automation_task_transition(&task, &request, Utc::now()).unwrap_err();

    assert!(matches!(
        error,
        AutomationTaskTransitionPolicyError::TaskAlreadyTerminal { .. }
    ));
}

#[test]
fn cancellation_plan_marks_cancel_request_and_terminal_timestamps() {
    let mut task = stored_task(AutomationTaskState::Running);
    let started_at = Utc::now() - chrono::Duration::seconds(5);
    task.started_at = Some(started_at);
    let now = Utc::now();

    let plan = plan_automation_task_cancellation(&task, now).unwrap();

    assert_eq!(plan.task_state, AutomationTaskState::Cancelled);
    assert_eq!(plan.task_cancel_requested_at, Some(now));
    assert_eq!(plan.task_started_at, Some(started_at));
    assert_eq!(plan.task_completed_at, Some(now));
    assert_eq!(plan.task_event_type, "automation_task.cancelled");
    assert_eq!(plan.task_log_stream, AutomationTaskLogStream::System);
    assert_eq!(plan.run_event_type, "workflow_run.cancelled");
    assert_eq!(plan.run_log_stream, AutomationTaskLogStream::System);
}

#[test]
fn cancellation_plan_rejects_terminal_task() {
    let task = stored_task(AutomationTaskState::Cancelled);

    let error = plan_automation_task_cancellation(&task, Utc::now()).unwrap_err();

    assert!(matches!(
        error,
        AutomationTaskTransitionPolicyError::TaskAlreadyTerminal { .. }
    ));
}
