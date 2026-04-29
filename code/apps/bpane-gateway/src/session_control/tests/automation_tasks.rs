use super::support::principal;
use super::*;

#[tokio::test]
async fn in_memory_store_tracks_automation_task_lifecycle_logs_and_events() {
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
                display_name: Some("demo task".to_string()),
                executor: "playwright".to_string(),
                session_id: session.id,
                session_source: AutomationTaskSessionSource::ExistingSession,
                input: Some(serde_json::json!({ "step": "login" })),
                labels: HashMap::from([("suite".to_string(), "workflow".to_string())]),
            },
        )
        .await
        .unwrap();
    assert_eq!(task.state, AutomationTaskState::Pending);

    let running = store
        .transition_automation_task(
            task.id,
            AutomationTaskTransitionRequest {
                state: AutomationTaskState::Running,
                output: None,
                error: None,
                artifact_refs: Vec::new(),
                event_type: "automation_task.running".to_string(),
                event_message: "task entered running state".to_string(),
                event_data: None,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(running.state, AutomationTaskState::Running);
    assert!(running.started_at.is_some());

    let log = store
        .append_automation_task_log(
            task.id,
            AutomationTaskLogStream::Stdout,
            "step 1 complete".to_string(),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(log.stream, AutomationTaskLogStream::Stdout);

    let succeeded = store
        .transition_automation_task(
            task.id,
            AutomationTaskTransitionRequest {
                state: AutomationTaskState::Succeeded,
                output: Some(serde_json::json!({ "result": "ok" })),
                error: None,
                artifact_refs: vec!["artifact://trace.zip".to_string()],
                event_type: "automation_task.succeeded".to_string(),
                event_message: "task completed successfully".to_string(),
                event_data: Some(serde_json::json!({ "duration_ms": 1200 })),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(succeeded.state, AutomationTaskState::Succeeded);
    assert!(succeeded.completed_at.is_some());
    assert_eq!(succeeded.artifact_refs.len(), 1);

    let events = store
        .list_automation_task_events_for_owner(&owner, task.id)
        .await
        .unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].event_type, "automation_task.created");
    assert_eq!(events[2].event_type, "automation_task.succeeded");

    let logs = store
        .list_automation_task_logs_for_owner(&owner, task.id)
        .await
        .unwrap();
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0].message, "step 1 complete");

    let error = store
        .transition_automation_task(
            task.id,
            AutomationTaskTransitionRequest {
                state: AutomationTaskState::Running,
                output: None,
                error: None,
                artifact_refs: Vec::new(),
                event_type: "automation_task.running".to_string(),
                event_message: "task should not resume".to_string(),
                event_data: None,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(error, SessionStoreError::Conflict(_)));
}
