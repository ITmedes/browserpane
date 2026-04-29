use super::*;

fn principal(subject: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        subject: subject.to_string(),
        issuer: "https://issuer.example".to_string(),
        display_name: Some(subject.to_string()),
        client_id: None,
    }
}

fn service_principal(subject: &str, client_id: &str) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        subject: subject.to_string(),
        issuer: "https://issuer.example".to_string(),
        display_name: Some(client_id.to_string()),
        client_id: Some(client_id.to_string()),
    }
}

#[tokio::test]
async fn in_memory_store_scopes_sessions_to_owner() {
    let store = SessionStore::in_memory();
    let alpha = principal("alpha");
    let bravo = principal("bravo");

    let created = store
        .create_session(
            &alpha,
            CreateSessionRequest {
                template_id: Some("default".to_string()),
                owner_mode: None,
                viewport: Some(SessionViewport {
                    width: 1920,
                    height: 1080,
                }),
                idle_timeout_sec: Some(600),
                labels: HashMap::from([("suite".to_string(), "smoke".to_string())]),
                integration_context: Some(serde_json::json!({ "ticket": "BPANE-6" })),
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy {
                    mode: SessionRecordingMode::Manual,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: Some(86_400),
                },
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let alpha_sessions = store.list_sessions_for_owner(&alpha).await.unwrap();
    assert_eq!(alpha_sessions.len(), 1);
    assert_eq!(alpha_sessions[0].id, created.id);
    assert_eq!(alpha_sessions[0].recording, created.recording);
    assert_eq!(created.recording.mode, SessionRecordingMode::Manual);
    assert_eq!(created.recording.format, SessionRecordingFormat::Webm);
    assert_eq!(created.recording.retention_sec, Some(86_400));

    let bravo_sessions = store.list_sessions_for_owner(&bravo).await.unwrap();
    assert!(bravo_sessions.is_empty());
    assert!(store
        .get_session_for_owner(&bravo, created.id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn in_memory_store_limits_legacy_runtime_to_one_active_session() {
    let store = SessionStore::in_memory();
    let alpha = principal("alpha");

    store
        .create_session(
            &alpha,
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

    let error = store
        .create_session(
            &alpha,
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
        .unwrap_err();

    assert!(matches!(
        error,
        SessionStoreError::ActiveSessionConflict {
            max_runtime_sessions: 1
        }
    ));
}

#[tokio::test]
async fn in_memory_store_respects_runtime_pool_capacity() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 2,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let alpha = principal("alpha");

    for _ in 0..2 {
        store
            .create_session(
                &alpha,
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
    }

    let error = store
        .create_session(
            &alpha,
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
        .unwrap_err();

    assert!(matches!(
        error,
        SessionStoreError::ActiveSessionConflict {
            max_runtime_sessions: 2
        }
    ));
}

#[tokio::test]
async fn in_memory_store_allows_delegated_client_to_load_session() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let delegate = service_principal("service-account-id", "bpane-mcp-bridge");

    let created = store
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

    let updated = store
        .set_automation_delegate_for_owner(
            &owner,
            created.id,
            SetAutomationDelegateRequest {
                client_id: "bpane-mcp-bridge".to_string(),
                issuer: None,
                display_name: Some("BrowserPane MCP bridge".to_string()),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        updated.automation_delegate.as_ref().unwrap().client_id,
        "bpane-mcp-bridge"
    );

    let visible = store
        .get_session_for_principal(&delegate, created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(visible.id, created.id);
}

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

#[tokio::test]
async fn in_memory_store_stops_unused_ready_sessions_and_idle_sessions() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let created = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: Some(300),
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

    let stopped_ready = store
        .stop_session_if_idle(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped_ready.state, SessionLifecycleState::Stopped);

    let created = store
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

    let active = store
        .mark_session_active(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.state, SessionLifecycleState::Active);

    let idle = store.mark_session_idle(created.id).await.unwrap().unwrap();
    assert_eq!(idle.state, SessionLifecycleState::Idle);

    let stopped = store
        .stop_session_if_idle(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, SessionLifecycleState::Stopped);

    let after = store
        .mark_session_active(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after.state, SessionLifecycleState::Stopped);
}

#[tokio::test]
async fn in_memory_store_can_prepare_a_stopped_session_for_reconnect() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let created = store
        .create_session(
            &owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: Some(300),
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

    let stopped = store
        .stop_session_if_idle(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, SessionLifecycleState::Stopped);

    let resumed = store
        .prepare_session_for_connect(created.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resumed.state, SessionLifecycleState::Ready);
    assert!(resumed.stopped_at.is_none());
}

#[tokio::test]
async fn reconnect_prep_respects_runtime_pool_capacity() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 1,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let owner = principal("owner");

    let ready = store
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
    assert_eq!(ready.state, SessionLifecycleState::Ready);

    let stopped = store
        .stop_session_for_owner(&owner, ready.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, SessionLifecycleState::Stopped);

    let replacement = store
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
    assert_eq!(replacement.state, SessionLifecycleState::Ready);

    let error = store
        .prepare_session_for_connect(stopped.id)
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        SessionStoreError::ActiveSessionConflict {
            max_runtime_sessions: 1
        }
    ));
}

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

#[tokio::test]
async fn in_memory_store_can_restore_runtime_candidate_to_ready_after_runtime_loss() {
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

    let active = store
        .mark_session_active(session.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(active.state, SessionLifecycleState::Active);

    let restored = store
        .mark_session_ready_after_runtime_loss(session.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored.state, SessionLifecycleState::Ready);
}

#[tokio::test]
async fn in_memory_store_creates_and_stops_recording_metadata() {
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
                    mode: SessionRecordingMode::Manual,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: None,
                },
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let created = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();
    assert_eq!(created.session_id, session.id);
    assert_eq!(created.previous_recording_id, None);
    assert_eq!(created.state, SessionRecordingState::Recording);
    assert_eq!(created.mime_type.as_deref(), Some("video/webm"));

    let listed = store.list_recordings_for_session(session.id).await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);

    let stopped = store
        .stop_recording_for_session(
            session.id,
            created.id,
            SessionRecordingTerminationReason::ManualStop,
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stopped.state, SessionRecordingState::Finalizing);
    assert_eq!(
        stopped.termination_reason,
        Some(SessionRecordingTerminationReason::ManualStop)
    );

    let latest = store
        .get_latest_recording_for_session(session.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.id, created.id);
    assert_eq!(latest.state, SessionRecordingState::Finalizing);
    assert_eq!(
        latest.termination_reason,
        Some(SessionRecordingTerminationReason::ManualStop)
    );

    let completed = store
        .complete_recording_for_session(
            session.id,
            created.id,
            PersistCompletedSessionRecordingRequest {
                artifact_ref: "local_fs:session/recording.webm".to_string(),
                mime_type: Some("video/webm".to_string()),
                bytes: Some(123),
                duration_ms: Some(456),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(completed.state, SessionRecordingState::Ready);
    assert_eq!(
        completed.artifact_ref.as_deref(),
        Some("local_fs:session/recording.webm")
    );
    assert_eq!(completed.bytes, Some(123));
    assert_eq!(completed.duration_ms, Some(456));

    let failed = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, Some(created.id))
        .await
        .unwrap();
    let failed = store
        .fail_recording_for_session(
            session.id,
            failed.id,
            FailSessionRecordingRequest {
                error: "boom".to_string(),
                termination_reason: Some(SessionRecordingTerminationReason::WorkerExit),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(failed.state, SessionRecordingState::Failed);
    assert_eq!(failed.previous_recording_id, Some(created.id));
    assert_eq!(failed.error.as_deref(), Some("boom"));
    assert_eq!(
        failed.termination_reason,
        Some(SessionRecordingTerminationReason::WorkerExit)
    );
}

#[tokio::test]
async fn in_memory_store_lists_and_clears_expired_recording_artifacts() {
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
                    mode: SessionRecordingMode::Manual,
                    format: SessionRecordingFormat::Webm,
                    retention_sec: Some(60),
                },
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let created = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();
    let completed = store
        .complete_recording_for_session(
            session.id,
            created.id,
            PersistCompletedSessionRecordingRequest {
                artifact_ref: "local_fs:session/recording.webm".to_string(),
                mime_type: Some("video/webm".to_string()),
                bytes: Some(123),
                duration_ms: Some(456),
            },
        )
        .await
        .unwrap()
        .unwrap();

    let candidates = store
        .list_recording_artifact_retention_candidates(
            completed.completed_at.unwrap() + chrono::Duration::seconds(61),
        )
        .await
        .unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].session_id, session.id);
    assert_eq!(candidates[0].recording_id, created.id);
    assert_eq!(
        candidates[0].artifact_ref,
        "local_fs:session/recording.webm"
    );

    let cleared = store
        .clear_recording_artifact_path(session.id, created.id)
        .await
        .unwrap()
        .unwrap();
    assert!(cleared.artifact_ref.is_none());

    let candidates = store
        .list_recording_artifact_retention_candidates(
            completed.completed_at.unwrap() + chrono::Duration::seconds(61),
        )
        .await
        .unwrap();
    assert!(candidates.is_empty());
}

#[test]
fn rejects_non_object_integration_context() {
    let error = validate_create_request(&CreateSessionRequest {
        template_id: None,
        owner_mode: None,
        viewport: None,
        idle_timeout_sec: None,
        labels: HashMap::new(),
        integration_context: Some(Value::String("bad".to_string())),
        extension_ids: Vec::new(),
        extensions: Vec::new(),
        recording: SessionRecordingPolicy::default(),
    })
    .unwrap_err();

    assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
}

#[test]
fn rejects_zero_recording_retention() {
    let error = validate_create_request(&CreateSessionRequest {
        template_id: None,
        owner_mode: None,
        viewport: None,
        idle_timeout_sec: None,
        labels: HashMap::new(),
        integration_context: None,
        extension_ids: Vec::new(),
        extensions: Vec::new(),
        recording: SessionRecordingPolicy {
            mode: SessionRecordingMode::Manual,
            format: SessionRecordingFormat::Webm,
            retention_sec: Some(0),
        },
    })
    .unwrap_err();

    assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
}

#[test]
fn rejects_empty_recording_failure_message() {
    let error = validate_fail_recording_request(&FailSessionRecordingRequest {
        error: "".to_string(),
        termination_reason: None,
    })
    .unwrap_err();

    assert!(matches!(error, SessionStoreError::InvalidRequest(_)));
}
