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
async fn in_memory_store_records_session_egress_usage_for_project_rollup() {
    let store = SessionStore::in_memory();
    let owner = principal("owner");
    let project = store
        .create_project(
            &owner,
            PersistProjectRequest {
                name: "support-egress".to_string(),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas {
                    max_active_sessions: None,
                    max_active_workflow_runs: None,
                    max_retained_storage_bytes: None,
                    max_session_creations: None,
                    max_session_creations_per_window: None,
                    session_creation_window_sec: None,
                    max_runtime_usage_ms: None,
                    max_egress_total_bytes: Some(100),
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

    let invalid = store
        .record_session_egress_usage(
            session.id,
            ReportSessionEgressUsageRequest {
                observer_id: Some("https://proxy.example".to_string()),
                ..ReportSessionEgressUsageRequest::default()
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(invalid, SessionStoreError::InvalidRequest(_)));

    let updated = store
        .record_session_egress_usage(
            session.id,
            ReportSessionEgressUsageRequest {
                rx_bytes_delta: 40,
                tx_bytes_delta: 30,
                source_kind: SessionEgressUsageSourceKind::Proxy,
                observer_id: Some("local-squid:3128".to_string()),
                observed_at: None,
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.egress_rx_bytes, 40);
    assert_eq!(updated.egress_tx_bytes, 30);

    let updated = store
        .record_session_egress_usage(
            session.id,
            ReportSessionEgressUsageRequest {
                rx_bytes_delta: 20,
                tx_bytes_delta: 20,
                source_kind: SessionEgressUsageSourceKind::TlsInterceptor,
                observer_id: Some("mitmproxy".to_string()),
                observed_at: Some(Utc::now()),
            },
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(updated.egress_rx_bytes, 60);
    assert_eq!(updated.egress_tx_bytes, 50);

    let (egress_rx_bytes, egress_tx_bytes) = store
        .sum_egress_usage_bytes_for_project(&owner, project.id)
        .await
        .unwrap();
    assert_eq!((egress_rx_bytes, egress_tx_bytes), (60, 50));

    let usage = project.usage(
        1,
        0,
        1,
        0,
        0,
        egress_rx_bytes,
        egress_tx_bytes,
        0,
        Utc::now(),
    );
    assert_eq!(usage.egress_total_bytes, 110);
    assert_eq!(usage.alerts.len(), 1);
    assert_eq!(
        usage.alerts[0].metric,
        ProjectUsageAlertMetric::EgressTotalBytes
    );
    assert_eq!(usage.alerts[0].state, ProjectUsageAlertState::Exceeded);
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
                    max_session_creations: None,
                    max_session_creations_per_window: None,
                    session_creation_window_sec: None,
                    max_runtime_usage_ms: None,
                    max_egress_total_bytes: None,
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
    tokio::time::sleep(std::time::Duration::from_millis(2)).await;

    let session_creations = store
        .count_session_creations_for_project(&owner, project.id)
        .await
        .unwrap();
    assert_eq!(session_creations, 1);
    let runtime_usage_ms = store
        .sum_runtime_usage_ms_for_project(&owner, project.id, Utc::now())
        .await
        .unwrap();
    assert!(runtime_usage_ms > 0);
    let egress_usage_bytes = store
        .sum_egress_usage_bytes_for_project(&owner, project.id)
        .await
        .unwrap();
    assert_eq!(egress_usage_bytes, (0, 0));

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

#[test]
fn project_usage_reports_soft_budget_alerts() {
    let now = Utc::now();
    let project = StoredProject {
        id: Uuid::now_v7(),
        owner_subject: "owner".to_string(),
        owner_issuer: "issuer".to_string(),
        name: "budgeted-project".to_string(),
        description: None,
        labels: HashMap::new(),
        quotas: ProjectQuotas {
            max_active_sessions: None,
            max_active_workflow_runs: None,
            max_retained_storage_bytes: None,
            max_session_creations: Some(1),
            max_session_creations_per_window: None,
            session_creation_window_sec: None,
            max_runtime_usage_ms: Some(10),
            max_egress_total_bytes: Some(100),
        },
        policy: ProjectPolicy::default(),
        state: ProjectState::Active,
        created_at: now,
        updated_at: now,
    };

    let usage = project.usage(0, 0, 1, 0, 8, 40, 40, 0, now);

    assert_eq!(usage.max_session_creations, Some(1));
    assert_eq!(usage.max_runtime_usage_ms, Some(10));
    assert_eq!(usage.max_egress_total_bytes, Some(100));
    assert_eq!(usage.alerts.len(), 3);
    assert!(usage.alerts.iter().any(|alert| {
        alert.metric == ProjectUsageAlertMetric::SessionCreations
            && alert.state == ProjectUsageAlertState::Exceeded
            && alert.current_value == 1
            && alert.limit_value == 1
    }));
    assert!(usage.alerts.iter().any(|alert| {
        alert.metric == ProjectUsageAlertMetric::RuntimeUsageMs
            && alert.state == ProjectUsageAlertState::ApproachingLimit
            && alert.current_value == 8
            && alert.limit_value == 10
    }));
    assert!(usage.alerts.iter().any(|alert| {
        alert.metric == ProjectUsageAlertMetric::EgressTotalBytes
            && alert.state == ProjectUsageAlertState::ApproachingLimit
            && alert.current_value == 80
            && alert.limit_value == 100
    }));
}

#[tokio::test]
async fn in_memory_store_blocks_session_creation_when_budget_enforcement_is_enabled() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 4,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let owner = principal("owner");
    let project = store
        .create_project(
            &owner,
            PersistProjectRequest {
                name: "blocking-budget".to_string(),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas {
                    max_active_sessions: None,
                    max_active_workflow_runs: None,
                    max_retained_storage_bytes: None,
                    max_session_creations: Some(1),
                    max_session_creations_per_window: None,
                    session_creation_window_sec: None,
                    max_runtime_usage_ms: Some(60_000),
                    max_egress_total_bytes: Some(1_048_576),
                },
                policy: ProjectPolicy {
                    usage_budget_enforcement: ProjectUsageBudgetEnforcement::BlockSessionCreation,
                    ..ProjectPolicy::default()
                },
                state: ProjectState::Active,
            },
        )
        .await
        .unwrap();

    store
        .create_session(
            &owner,
            project_session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let rejected = store
        .create_session(
            &owner,
            project_session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(rejected, SessionStoreError::Conflict(message) if message.contains("session_creation_budget_exceeded"))
    );

    let session_creations = store
        .count_session_creations_for_project(&owner, project.id)
        .await
        .unwrap();
    assert_eq!(session_creations, 1);
}

#[tokio::test]
async fn in_memory_store_blocks_session_creation_when_rate_limit_is_exhausted() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 4,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let owner = principal("owner");
    let project = store
        .create_project(
            &owner,
            PersistProjectRequest {
                name: "rate-limited-project".to_string(),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas {
                    max_active_sessions: None,
                    max_active_workflow_runs: None,
                    max_retained_storage_bytes: None,
                    max_session_creations: None,
                    max_session_creations_per_window: Some(1),
                    session_creation_window_sec: Some(3_600),
                    max_runtime_usage_ms: None,
                    max_egress_total_bytes: None,
                },
                policy: ProjectPolicy {
                    usage_budget_enforcement: ProjectUsageBudgetEnforcement::BlockSessionCreation,
                    ..ProjectPolicy::default()
                },
                state: ProjectState::Active,
            },
        )
        .await
        .unwrap();

    store
        .create_session(
            &owner,
            project_session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();

    let rejected = store
        .create_session(
            &owner,
            project_session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(rejected, SessionStoreError::Conflict(message) if message.contains("session_creation_rate_exceeded"))
    );

    let session_creations = store
        .count_session_creations_for_project(&owner, project.id)
        .await
        .unwrap();
    assert_eq!(session_creations, 1);
}

#[tokio::test]
async fn in_memory_store_keeps_runtime_budget_advisory_when_warning_only() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 4,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let owner = principal("owner");
    let project = store
        .create_project(
            &owner,
            PersistProjectRequest {
                name: "runtime-warning-project".to_string(),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas {
                    max_active_sessions: None,
                    max_active_workflow_runs: None,
                    max_retained_storage_bytes: None,
                    max_session_creations: None,
                    max_session_creations_per_window: None,
                    session_creation_window_sec: None,
                    max_runtime_usage_ms: Some(1),
                    max_egress_total_bytes: None,
                },
                policy: ProjectPolicy::default(),
                state: ProjectState::Active,
            },
        )
        .await
        .unwrap();

    store
        .create_session(
            &owner,
            project_session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let runtime_usage_ms = store
        .sum_runtime_usage_ms_for_project(&owner, project.id, Utc::now())
        .await
        .unwrap();
    assert!(runtime_usage_ms >= 1);

    store
        .create_session(
            &owner,
            project_session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn in_memory_store_blocks_session_creation_when_runtime_budget_is_exhausted() {
    let store = SessionStore::in_memory_with_config(SessionManagerProfile {
        runtime_binding: "docker_runtime_pool".to_string(),
        compatibility_mode: "session_runtime_pool".to_string(),
        max_runtime_sessions: 4,
        supports_legacy_global_routes: false,
        supports_session_extensions: true,
    });
    let owner = principal("owner");
    let project = store
        .create_project(
            &owner,
            PersistProjectRequest {
                name: "runtime-blocking-project".to_string(),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas {
                    max_active_sessions: None,
                    max_active_workflow_runs: None,
                    max_retained_storage_bytes: None,
                    max_session_creations: None,
                    max_session_creations_per_window: None,
                    session_creation_window_sec: None,
                    max_runtime_usage_ms: Some(1),
                    max_egress_total_bytes: None,
                },
                policy: ProjectPolicy {
                    usage_budget_enforcement: ProjectUsageBudgetEnforcement::BlockSessionCreation,
                    ..ProjectPolicy::default()
                },
                state: ProjectState::Active,
            },
        )
        .await
        .unwrap();

    store
        .create_session(
            &owner,
            project_session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    let rejected = store
        .create_session(
            &owner,
            project_session_request(project.id),
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap_err();
    assert!(
        matches!(rejected, SessionStoreError::Conflict(message) if message.contains("runtime_usage_budget_exceeded"))
    );

    let session_creations = store
        .count_session_creations_for_project(&owner, project.id)
        .await
        .unwrap();
    assert_eq!(session_creations, 1);
}
