use super::support::principal;
use super::*;

fn project_session_request(project_id: Uuid) -> CreateSessionRequest {
    CreateSessionRequest {
        project_id: Some(project_id),
        template_id: None,
        browser_context: None,
        network_identity: None,
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
    }
}

#[tokio::test]
async fn in_memory_store_reports_and_enforces_project_retained_storage() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let project = store
        .create_project(
            &owner,
            PersistProjectRequest {
                name: "support-storage".to_string(),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas {
                    max_active_sessions: None,
                    max_active_workflow_runs: None,
                    max_retained_storage_bytes: Some(40),
                },
                policy: ProjectPolicy::default(),
                state: ProjectState::Active,
            },
        )
        .await
        .unwrap();
    let session = store
        .create_session(
            &owner,
            project_session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    store
        .record_session_file(PersistSessionFileRequest {
            id: Uuid::now_v7(),
            session_id: session.id,
            name: "upload.txt".to_string(),
            media_type: Some("text/plain".to_string()),
            byte_count: 11,
            sha256_hex: "a".repeat(64),
            artifact_ref: "local_fs:session/upload.txt".to_string(),
            source: SessionFileSource::BrowserUpload,
            labels: HashMap::new(),
        })
        .await
        .unwrap();

    let recording = store
        .create_recording_for_session(session.id, SessionRecordingFormat::Webm, None)
        .await
        .unwrap();
    store
        .complete_recording_for_session(
            session.id,
            recording.id,
            PersistCompletedSessionRecordingRequest {
                artifact_ref: "local_fs:session/recording.webm".to_string(),
                mime_type: Some("video/webm".to_string()),
                bytes: Some(12),
                duration_ms: Some(1000),
            },
        )
        .await
        .unwrap();

    let task = store
        .create_automation_task(
            &owner,
            PersistAutomationTaskRequest {
                display_name: Some("retained storage task".to_string()),
                executor: "playwright".to_string(),
                session_id: session.id,
                session_source: AutomationTaskSessionSource::ExistingSession,
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
                name: "retained-storage-workflow".to_string(),
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
                entrypoint: "workflow.mjs".to_string(),
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
                workflow_version: version.version,
                project_id: Some(project.id),
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
        .append_workflow_run_produced_file(
            run.id,
            PersistWorkflowRunProducedFileRequest {
                workspace_id: Uuid::now_v7(),
                file_id: Uuid::now_v7(),
                file_name: "result.json".to_string(),
                media_type: Some("application/json".to_string()),
                byte_count: 13,
                sha256_hex: "b".repeat(64),
                provenance: None,
                artifact_ref: "local_fs:workflow/result.json".to_string(),
            },
        )
        .await
        .unwrap();

    let retained_storage_bytes = store
        .sum_retained_storage_bytes_for_project(&owner, project.id)
        .await
        .unwrap();
    assert_eq!(retained_storage_bytes, 36);

    let over_quota = store
        .record_session_file(PersistSessionFileRequest {
            id: Uuid::now_v7(),
            session_id: session.id,
            name: "too-large.txt".to_string(),
            media_type: Some("text/plain".to_string()),
            byte_count: 5,
            sha256_hex: "c".repeat(64),
            artifact_ref: "local_fs:session/too-large.txt".to_string(),
            source: SessionFileSource::BrowserUpload,
            labels: HashMap::new(),
        })
        .await
        .unwrap_err();
    assert!(matches!(
        over_quota,
        SessionStoreError::Conflict(message)
            if message.contains("retained_storage_quota_exceeded")
    ));
}
