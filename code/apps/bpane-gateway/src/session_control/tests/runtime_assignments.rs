use super::support::principal;
use super::*;

#[tokio::test]
async fn in_memory_store_persists_runtime_assignments_and_can_clear_them() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 2,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
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

    store
        .upsert_runtime_assignment(PersistedSessionRuntimeAssignment {
            session_id: session.id,
            runtime_binding: "docker_runtime_pool".to_string(),
            status: SessionRuntimeAssignmentStatus::Ready,
            agent_socket_path: format!("/run/bpane/sessions/{}.sock", session.id),
            container_name: Some(format!("bpane-runtime-{}", session.id.as_simple())),
            cdp_endpoint: Some(format!(
                "http://bpane-runtime-{}:9223",
                session.id.as_simple()
            )),
        })
        .await
        .unwrap();

    let assignments = store
        .list_runtime_assignments("docker_runtime_pool")
        .await
        .unwrap();
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].session_id, session.id);
    assert_eq!(assignments[0].status, SessionRuntimeAssignmentStatus::Ready);

    store.clear_runtime_assignment(session.id).await.unwrap();
    assert!(store
        .list_runtime_assignments("docker_runtime_pool")
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn in_memory_store_persists_recording_worker_assignments() {
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
                recording: SessionRecordingPolicy {
                    mode: SessionRecordingMode::Always,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: None,
                },
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let recording = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();

    store
        .upsert_recording_worker_assignment(PersistedSessionRecordingWorkerAssignment {
            session_id: session.id,
            recording_id: recording.id,
            status: SessionRecordingWorkerAssignmentStatus::Running,
            process_id: Some(4242),
        })
        .await
        .unwrap();

    let loaded = store
        .get_recording_worker_assignment(session.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.recording_id, recording.id);
    assert_eq!(
        loaded.status,
        SessionRecordingWorkerAssignmentStatus::Running
    );
    assert_eq!(loaded.process_id, Some(4242));

    let listed = store.list_recording_worker_assignments().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].session_id, session.id);

    store
        .clear_recording_worker_assignment(session.id)
        .await
        .unwrap();
    assert!(store
        .list_recording_worker_assignments()
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn in_memory_store_persists_workflow_run_worker_assignments() {
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
                name: "Smoke Workflow".to_string(),
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
                entrypoint: "workflows/smoke/run.mjs".to_string(),
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
        .upsert_workflow_run_worker_assignment(PersistedWorkflowRunWorkerAssignment {
            run_id: run.id,
            session_id: session.id,
            automation_task_id: task.id,
            status: WorkflowRunWorkerAssignmentStatus::Running,
            process_id: Some(5151),
            container_name: Some("bpane-workflow-test".to_string()),
        })
        .await
        .unwrap();

    let loaded = store
        .get_workflow_run_worker_assignment(run.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(loaded.session_id, session.id);
    assert_eq!(loaded.automation_task_id, task.id);
    assert_eq!(loaded.status, WorkflowRunWorkerAssignmentStatus::Running);
    assert_eq!(loaded.process_id, Some(5151));
    assert_eq!(
        loaded.container_name.as_deref(),
        Some("bpane-workflow-test")
    );

    let listed = store.list_workflow_run_worker_assignments().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].run_id, run.id);

    store
        .clear_workflow_run_worker_assignment(run.id)
        .await
        .unwrap();
    assert!(store
        .list_workflow_run_worker_assignments()
        .await
        .unwrap()
        .is_empty());
}
