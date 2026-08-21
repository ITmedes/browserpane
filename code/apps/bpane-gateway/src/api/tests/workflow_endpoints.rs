use super::*;
use crate::session_control::ProjectQuotas;
use crate::workflow_endpoints::{
    PersistWorkflowEndpointGrantRequest, PersistWorkflowEndpointRequest,
    WorkflowEndpointArtifactBehavior, WorkflowEndpointGrantOperation, WorkflowEndpointState,
};

struct MachineFixture {
    app: Router,
    token: String,
    state: Arc<ApiState>,
    owner: AuthenticatedPrincipal,
    project_id: Uuid,
    endpoint_key: String,
}

#[tokio::test]
async fn owner_manages_endpoint_lifecycle_and_grants_with_problem_details() {
    let (app, token, state) = test_router_with_state();
    let owner = state
        .auth_validator
        .authenticate(&token)
        .await
        .expect("test owner should authenticate");
    let (project, definition, version) = create_endpoint_dependencies(&state, &owner, true).await;
    let service_principal = state
        .session_store
        .create_service_principal(
            &owner,
            endpoint_service_principal_request(project.id, "owner-lifecycle-client"),
        )
        .await
        .expect("endpoint service principal should be created");
    let endpoint_uri = format!(
        "/api/v1/projects/{}/workflow-endpoints/retrieve-supplier-report",
        project.id
    );

    let invalid = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/projects/{}/workflow-endpoints",
                    project.id
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "endpoint_key": "retrieve-supplier-report",
                        "purpose": "Retrieve one supplier report",
                        "workflow_definition_id": definition.id,
                        "workflow_definition_version_id": version.id,
                        "workflow_version": version.version,
                        "input_schema": { "$schema": "http://json-schema.org/draft-07/schema#" },
                        "output_schema": declared_schema(json!({ "ok": { "type": "boolean" } }), ["ok"])
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        invalid.headers()["content-type"],
        "application/problem+json"
    );
    let invalid_body = response_json(invalid).await;
    assert_eq!(invalid_body["code"], "invalid_json_schema");
    assert!(invalid_body["errors"]
        .as_array()
        .is_some_and(|errors| !errors.is_empty()));

    let created = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/projects/{}/workflow-endpoints",
                    project.id
                ))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    endpoint_payload(&definition, &version, 30).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(created.status(), StatusCode::CREATED);
    let created_body = response_json(created).await;
    assert_eq!(created_body["state"], "draft");
    assert_eq!(
        created_body["supported_controls"],
        json!(["poll", "cancel"])
    );

    let grant = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("{endpoint_uri}/grants"))
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "service_principal_id": service_principal.id,
                        "operations": ["invoke", "read", "cancel", "artifact.read"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(grant.status(), StatusCode::OK);
    let grant_body = response_json(grant).await;
    let grant_id = grant_body["id"].as_str().unwrap();

    let activated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("{endpoint_uri}/activate"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(activated.status(), StatusCode::OK);
    assert_eq!(response_json(activated).await["state"], "active");

    let blocked_update = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&endpoint_uri)
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    endpoint_payload(&definition, &version, 45).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked_update.status(), StatusCode::CONFLICT);
    assert_eq!(
        blocked_update.headers()["content-type"],
        "application/problem+json"
    );

    let disabled = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("{endpoint_uri}/disable"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(disabled.status(), StatusCode::OK);
    assert_eq!(response_json(disabled).await["state"], "disabled");

    let updated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(&endpoint_uri)
                .header("authorization", bearer(&token))
                .header("content-type", "application/json")
                .body(Body::from(
                    endpoint_payload(&definition, &version, 45).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(updated.status(), StatusCode::OK);
    assert_eq!(
        response_json(updated).await["execution_timeout_seconds"],
        45
    );

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/projects/{}/workflow-endpoints",
                    project.id
                ))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(
        response_json(listed).await["workflow_endpoints"][0]["endpoint_key"],
        "retrieve-supplier-report"
    );

    let revoked = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("{endpoint_uri}/grants/{grant_id}"))
                .header("authorization", bearer(&token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn machine_invocation_validates_before_side_effect_and_enforces_idempotency_and_cancel() {
    let fixture = machine_fixture(30).await;
    let invalid = invoke(&fixture, "invalid-input", json!({ "wrong": true })).await;
    assert_eq!(invalid.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        invalid.headers()["content-type"],
        "application/problem+json"
    );
    assert_eq!(
        response_json(invalid).await["code"],
        "input_schema_validation_failed"
    );
    assert!(fixture
        .state
        .session_store
        .list_sessions_for_owner(&fixture.owner)
        .await
        .unwrap()
        .is_empty());

    let accepted = invoke(
        &fixture,
        "same-request",
        json!({ "reporting_period": "2026-Q3" }),
    )
    .await;
    assert_eq!(accepted.status(), StatusCode::ACCEPTED);
    let accepted_body = response_json(accepted).await;
    let invocation_id = accepted_body["id"].as_str().unwrap().to_string();
    let run_id = accepted_body["run_id"].as_str().unwrap().to_string();
    assert_eq!(accepted_body["state"], "pending");
    assert!(accepted_body.get("labels").is_none());
    assert!(accepted_body.get("credentials").is_none());
    assert!(accepted_body.get("logs").is_none());

    let replay = invoke(
        &fixture,
        "same-request",
        json!({ "reporting_period": "2026-Q3" }),
    )
    .await;
    assert_eq!(replay.status(), StatusCode::OK);
    let replay_body = response_json(replay).await;
    assert_eq!(replay_body["id"], invocation_id);
    assert_eq!(replay_body["run_id"], run_id);

    let conflict = invoke(
        &fixture,
        "same-request",
        json!({ "reporting_period": "2026-Q4" }),
    )
    .await;
    assert_eq!(conflict.status(), StatusCode::CONFLICT);
    assert_eq!(
        response_json(conflict).await["code"],
        "idempotency_key_conflict"
    );

    let cancelled = fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "{}/cancel",
                    invocation_path(&fixture, &invocation_id)
                ))
                .header("authorization", bearer(&fixture.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cancelled.status(), StatusCode::OK);
    let cancelled_body = response_json(cancelled).await;
    assert_eq!(cancelled_body["state"], "cancelled");
    assert_eq!(cancelled_body["outcome"]["category"], "cancellation");
    assert_eq!(cancelled_body["side_effect_state"], "none");

    let disabled_principal = fixture
        .state
        .session_store
        .list_service_principals_for_owner(&fixture.owner)
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    fixture
        .state
        .session_store
        .update_service_principal_for_owner(
            &fixture.owner,
            disabled_principal.id,
            PersistServicePrincipalRequest {
                name: disabled_principal.name,
                description: disabled_principal.description,
                client_id: disabled_principal.client_id,
                issuer: disabled_principal.issuer,
                labels: disabled_principal.labels,
                scopes: disabled_principal.scopes,
                allowed_project_ids: disabled_principal.allowed_project_ids,
                state: ServicePrincipalState::Disabled,
            },
        )
        .await
        .unwrap();
    let denied = invoke(
        &fixture,
        "disabled-principal",
        json!({ "reporting_period": "2026-Q3" }),
    )
    .await;
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        response_json(denied).await["code"],
        "workflow_endpoint_authorization_denied"
    );
}

#[tokio::test]
async fn polling_maps_deadline_and_challenge_to_explicit_terminal_outcomes() {
    let timeout_fixture = machine_fixture(1).await;
    let accepted = invoke(
        &timeout_fixture,
        "timeout",
        json!({ "reporting_period": "2026-Q3" }),
    )
    .await;
    let timeout_body = response_json(accepted).await;
    let timeout_invocation_id = timeout_body["id"].as_str().unwrap().to_string();
    let timeout_run = run_for_resource(&timeout_fixture, &timeout_body).await;
    transition_task(
        &timeout_fixture.state,
        &timeout_run,
        AutomationTaskState::Queued,
        None,
    )
    .await;
    transition_task(
        &timeout_fixture.state,
        &timeout_run,
        AutomationTaskState::Running,
        None,
    )
    .await;
    sleep(Duration::from_millis(1_100)).await;
    let timed_out = get_invocation(&timeout_fixture, &timeout_invocation_id).await;
    assert_eq!(timed_out.status(), StatusCode::OK);
    let timed_out_body = response_json(timed_out).await;
    assert_eq!(timed_out_body["state"], "timed_out");
    assert_eq!(timed_out_body["outcome"]["category"], "timeout");
    assert_eq!(timed_out_body["outcome"]["retryable"], false);
    assert_eq!(timed_out_body["side_effect_state"], "uncertain");

    let challenge_fixture = machine_fixture(30).await;
    let accepted = invoke(
        &challenge_fixture,
        "challenge",
        json!({ "reporting_period": "2026-Q3" }),
    )
    .await;
    let challenge_body = response_json(accepted).await;
    let challenge_invocation_id = challenge_body["id"].as_str().unwrap().to_string();
    let challenge_run = run_for_resource(&challenge_fixture, &challenge_body).await;
    transition_task(
        &challenge_fixture.state,
        &challenge_run,
        AutomationTaskState::Queued,
        None,
    )
    .await;
    transition_task(
        &challenge_fixture.state,
        &challenge_run,
        AutomationTaskState::Running,
        None,
    )
    .await;
    transition_task(
        &challenge_fixture.state,
        &challenge_run,
        AutomationTaskState::AwaitingInput,
        None,
    )
    .await;
    let challenge = get_invocation(&challenge_fixture, &challenge_invocation_id).await;
    let challenge = response_json(challenge).await;
    assert_eq!(challenge["state"], "failed");
    assert_eq!(
        challenge["outcome"]["category"],
        "external_intervention_required"
    );
    assert_eq!(
        challenge["outcome"]["code"],
        "external_intervention_required"
    );
    assert_eq!(challenge["side_effect_state"], "uncertain");
}

#[tokio::test]
async fn invalid_output_cannot_succeed_and_artifacts_require_grants_and_availability() {
    let invalid_output_fixture = machine_fixture(30).await;
    let accepted = invoke(
        &invalid_output_fixture,
        "invalid-output",
        json!({ "reporting_period": "2026-Q3" }),
    )
    .await;
    let body = response_json(accepted).await;
    let invocation_id = body["id"].as_str().unwrap().to_string();
    let run = run_for_resource(&invalid_output_fixture, &body).await;
    transition_task(
        &invalid_output_fixture.state,
        &run,
        AutomationTaskState::Queued,
        None,
    )
    .await;
    transition_task(
        &invalid_output_fixture.state,
        &run,
        AutomationTaskState::Running,
        None,
    )
    .await;
    transition_task(
        &invalid_output_fixture.state,
        &run,
        AutomationTaskState::Succeeded,
        Some(json!({ "ok": "not-a-boolean" })),
    )
    .await;
    let invalid_output =
        response_json(get_invocation(&invalid_output_fixture, &invocation_id).await).await;
    assert_eq!(invalid_output["state"], "failed");
    assert!(invalid_output["result"].is_null());
    assert_eq!(invalid_output["outcome"]["category"], "validation_failure");

    let artifact_fixture = machine_fixture(30).await;
    let accepted = invoke(
        &artifact_fixture,
        "artifact",
        json!({ "reporting_period": "2026-Q3" }),
    )
    .await;
    let body = response_json(accepted).await;
    let artifact_invocation_id = body["id"].as_str().unwrap().to_string();
    let artifact_run = run_for_resource(&artifact_fixture, &body).await;
    transition_task(
        &artifact_fixture.state,
        &artifact_run,
        AutomationTaskState::Queued,
        None,
    )
    .await;
    transition_task(
        &artifact_fixture.state,
        &artifact_run,
        AutomationTaskState::Running,
        None,
    )
    .await;
    let bytes = b"supplier-report".to_vec();
    let workspace_id = Uuid::now_v7();
    let file_id = Uuid::now_v7();
    let stored = artifact_fixture
        .state
        .workspace_file_store
        .write(crate::workspaces::StoreWorkspaceFileRequest {
            workspace_id,
            file_id,
            file_name: "supplier-report.txt".to_string(),
            bytes: bytes.clone(),
        })
        .await
        .unwrap();
    artifact_fixture
        .state
        .session_store
        .append_workflow_run_produced_file(
            artifact_run.id,
            PersistWorkflowRunProducedFileRequest {
                workspace_id,
                file_id,
                file_name: "supplier-report.txt".to_string(),
                media_type: Some("text/plain".to_string()),
                byte_count: bytes.len() as u64,
                sha256_hex: hex::encode(Sha256::digest(&bytes)),
                provenance: Some(json!({ "source": "fixture" })),
                artifact_ref: stored.artifact_ref.clone(),
            },
        )
        .await
        .unwrap();
    transition_task(
        &artifact_fixture.state,
        &artifact_run,
        AutomationTaskState::Succeeded,
        Some(json!({ "ok": true })),
    )
    .await;
    let resource =
        response_json(get_invocation(&artifact_fixture, &artifact_invocation_id).await).await;
    assert_eq!(resource["state"], "succeeded");
    assert_eq!(resource["result"], json!({ "ok": true }));
    assert_eq!(resource["artifacts"][0]["file_id"], file_id.to_string());
    assert_eq!(resource["artifacts"][0]["media_type"], "text/plain");
    assert_eq!(resource["artifacts"][0]["byte_count"], bytes.len());
    assert!(resource["artifacts"][0]["content_path"].as_str().is_some());
    let content_path = resource["artifacts"][0]["content_path"]
        .as_str()
        .unwrap()
        .to_string();
    let content = artifact_fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&content_path)
                .header("authorization", bearer(&artifact_fixture.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(content.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(content.into_body(), usize::MAX).await.unwrap(),
        bytes
    );
    artifact_fixture
        .state
        .workspace_file_store
        .delete(&stored.artifact_ref)
        .await
        .unwrap();
    let unavailable = artifact_fixture
        .app
        .oneshot(
            Request::builder()
                .uri(&content_path)
                .header("authorization", bearer(&artifact_fixture.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unavailable.status(), StatusCode::GONE);
    assert_eq!(
        response_json(unavailable).await["code"],
        "artifact_unavailable"
    );
}

async fn machine_fixture(execution_timeout_seconds: u32) -> MachineFixture {
    let caller = AuthenticatedPrincipal {
        subject: "service-account-fake-bpm".to_string(),
        issuer: "https://machine-issuer.example".to_string(),
        display_name: Some("fake-bpm".to_string()),
        client_id: Some("fake-bpm".to_string()),
        safe_claims: Default::default(),
    };
    let (app, token, state) = test_router_with_principal(caller);
    let owner = AuthenticatedPrincipal {
        subject: "workflow-endpoint-owner".to_string(),
        issuer: "https://owner-issuer.example".to_string(),
        display_name: None,
        client_id: None,
        safe_claims: Default::default(),
    };
    let (project, definition, version) = create_endpoint_dependencies(&state, &owner, false).await;
    let service_principal = state
        .session_store
        .create_service_principal(
            &owner,
            endpoint_service_principal_request(project.id, "fake-bpm"),
        )
        .await
        .unwrap();
    let endpoint_key = "retrieve-supplier-report".to_string();
    let endpoint = state
        .session_store
        .create_workflow_endpoint(
            &owner,
            PersistWorkflowEndpointRequest {
                project_id: project.id,
                endpoint_key: endpoint_key.clone(),
                purpose: "Retrieve one supplier report".to_string(),
                workflow_definition_id: definition.id,
                workflow_definition_version_id: version.id,
                workflow_version: version.version,
                input_schema: declared_schema(
                    json!({ "reporting_period": { "type": "string" } }),
                    ["reporting_period"],
                ),
                output_schema: declared_schema(json!({ "ok": { "type": "boolean" } }), ["ok"]),
                execution_timeout_seconds,
                inline_result_max_bytes: 65_536,
                artifact_behavior: WorkflowEndpointArtifactBehavior {
                    mode: "authorized_references".to_string(),
                    retention_seconds: 60,
                },
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    state
        .session_store
        .upsert_workflow_endpoint_grant_for_owner(
            &owner,
            &endpoint,
            PersistWorkflowEndpointGrantRequest {
                service_principal_id: service_principal.id,
                operations: vec![
                    WorkflowEndpointGrantOperation::Invoke,
                    WorkflowEndpointGrantOperation::Read,
                    WorkflowEndpointGrantOperation::Cancel,
                    WorkflowEndpointGrantOperation::ArtifactRead,
                ],
            },
        )
        .await
        .unwrap();
    state
        .session_store
        .set_workflow_endpoint_state_for_owner(&owner, endpoint.id, WorkflowEndpointState::Active)
        .await
        .unwrap();
    MachineFixture {
        app,
        token,
        state,
        owner,
        project_id: project.id,
        endpoint_key,
    }
}

async fn create_endpoint_dependencies(
    state: &ApiState,
    owner: &AuthenticatedPrincipal,
    supported_package: bool,
) -> (
    StoredProject,
    StoredWorkflowDefinition,
    StoredWorkflowDefinitionVersion,
) {
    let project = state
        .session_store
        .create_project(
            owner,
            PersistProjectRequest {
                name: format!("endpoint-project-{}", Uuid::now_v7()),
                description: None,
                labels: HashMap::new(),
                quotas: ProjectQuotas::default(),
                policy: ProjectPolicy::default(),
                state: ProjectState::Active,
            },
        )
        .await
        .unwrap();
    let definition = state
        .session_store
        .create_workflow_definition(
            owner,
            PersistWorkflowDefinitionRequest {
                name: format!("endpoint-workflow-{}", Uuid::now_v7()),
                description: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let package = supported_package.then(|| endpoint_package(project.id));
    let source = supported_package.then(|| {
        serde_json::from_value(json!({
            "kind": "git",
            "repository_url": "https://example.test/workflow.git",
            "ref": "main",
            "resolved_commit": "a".repeat(40),
            "root_path": "workflow"
        }))
        .unwrap()
    });
    let version = state
        .session_store
        .create_workflow_definition_version(
            owner,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: definition.id,
                version: "v1".to_string(),
                executor: "playwright".to_string(),
                entrypoint: "workflow/run.ts".to_string(),
                source,
                input_schema: Some(declared_schema(
                    json!({ "reporting_period": { "type": "string" } }),
                    ["reporting_period"],
                )),
                output_schema: Some(declared_schema(
                    json!({ "ok": { "type": "boolean" } }),
                    ["ok"],
                )),
                package,
                default_session: Some(json!({})),
                allowed_credential_binding_ids: Vec::new(),
                allowed_extension_ids: Vec::new(),
                allowed_file_workspace_ids: Vec::new(),
            },
        )
        .await
        .unwrap();
    (project, definition, version)
}

fn endpoint_service_principal_request(
    project_id: Uuid,
    client_id: &str,
) -> PersistServicePrincipalRequest {
    PersistServicePrincipalRequest {
        name: "Fake BPM".to_string(),
        description: None,
        client_id: client_id.to_string(),
        issuer: "https://machine-issuer.example".to_string(),
        labels: HashMap::new(),
        scopes: vec![
            "workflow-endpoints:invoke".to_string(),
            "workflow-endpoints:read".to_string(),
            "workflow-endpoints:cancel".to_string(),
            "workflow-endpoints:artifact.read".to_string(),
        ],
        allowed_project_ids: vec![project_id],
        state: ServicePrincipalState::Active,
    }
}

fn endpoint_payload(
    definition: &StoredWorkflowDefinition,
    version: &StoredWorkflowDefinitionVersion,
    execution_timeout_seconds: u32,
) -> Value {
    json!({
        "endpoint_key": "retrieve-supplier-report",
        "purpose": "Retrieve one supplier report",
        "workflow_definition_id": definition.id,
        "workflow_definition_version_id": version.id,
        "workflow_version": version.version,
        "input_schema": declared_schema(
            json!({ "reporting_period": { "type": "string" } }),
            ["reporting_period"]
        ),
        "output_schema": declared_schema(json!({ "ok": { "type": "boolean" } }), ["ok"]),
        "execution_timeout_seconds": execution_timeout_seconds,
        "inline_result_max_bytes": 65536,
        "artifact_behavior": { "mode": "authorized_references", "retention_seconds": 60 },
        "labels": { "fixture": "endpoint" }
    })
}

fn endpoint_package(project_id: Uuid) -> WorkflowPackageManifest {
    serde_json::from_value(json!({
        "package_id": "browserpane.endpoint-test.v1",
        "format_version": "browserpane.workflow-package/v1",
        "runtime": {
            "language": "typescript",
            "browserpane_api_version": "v1",
            "node_major_version": 22,
            "playwright_major_version": 1,
            "playwright_minor_version": 59
        },
        "requirements": {
            "default_session": {
                "project_id": project_id,
                "browser_context": { "mode": "fresh", "context_id": null },
                "network_identity": { "egress_profile_id": null },
                "capabilities": {
                    "browser_input": true,
                    "clipboard": false,
                    "audio": false,
                    "microphone": false,
                    "camera": false,
                    "file_transfer": false,
                    "resize": true
                },
                "recording": { "mode": "disabled", "format": "webm", "retention_sec": null },
                "extension_ids": []
            },
            "allowed_credential_binding_ids": [],
            "allowed_extension_ids": [],
            "allowed_file_workspace_ids": []
        },
        "execution": {
            "timeout_ms": 60000,
            "assertions": ["schema-valid-output"],
            "safe_cancellation_points": ["before-submit"],
            "side_effect_checkpoints": ["after-submit"]
        },
        "publication": {
            "reviewer": "endpoint-test-reviewer",
            "reviewed_at": "2026-08-21T00:00:00Z",
            "decision": "approved",
            "fresh_context_replay": true,
            "scenarios": [
                { "kind": "happy_path", "result": "passed" },
                { "kind": "validation", "result": "passed" },
                { "kind": "missing_element", "result": "passed" },
                { "kind": "authentication_challenge", "result": "passed" },
                { "kind": "portal_failure", "result": "passed" },
                { "kind": "runtime_failure", "result": "passed" },
                { "kind": "cancellation", "result": "passed" },
                { "kind": "ambiguous_post_side_effect", "result": "passed" }
            ]
        }
    }))
    .unwrap()
}

fn declared_schema<const N: usize>(properties: Value, required: [&str; N]) -> Value {
    let required = required.into_iter().collect::<Vec<_>>();
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "required": required,
        "properties": properties,
        "additionalProperties": false
    })
}

async fn invoke(fixture: &MachineFixture, key: &str, input: Value) -> axum::response::Response {
    fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/v1/projects/{}/workflow-endpoints/{}/invocations",
                    fixture.project_id, fixture.endpoint_key
                ))
                .header("authorization", bearer(&fixture.token))
                .header("idempotency-key", key)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "input": input,
                        "source_system": "fake-bpm",
                        "source_reference": format!("process-{key}")
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}

fn invocation_path(fixture: &MachineFixture, invocation_id: &str) -> String {
    format!(
        "/api/v1/projects/{}/workflow-endpoints/{}/invocations/{invocation_id}",
        fixture.project_id, fixture.endpoint_key
    )
}

async fn get_invocation(fixture: &MachineFixture, invocation_id: &str) -> axum::response::Response {
    fixture
        .app
        .clone()
        .oneshot(
            Request::builder()
                .uri(invocation_path(fixture, invocation_id))
                .header("authorization", bearer(&fixture.token))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn run_for_resource(fixture: &MachineFixture, resource: &Value) -> StoredWorkflowRun {
    fixture
        .state
        .session_store
        .get_workflow_run_by_id(Uuid::parse_str(resource["run_id"].as_str().unwrap()).unwrap())
        .await
        .unwrap()
        .unwrap()
}

async fn transition_task(
    state: &ApiState,
    run: &StoredWorkflowRun,
    task_state: AutomationTaskState,
    output: Option<Value>,
) {
    state
        .session_store
        .transition_automation_task(
            run.automation_task_id,
            AutomationTaskTransitionRequest {
                state: task_state,
                output,
                error: None,
                artifact_refs: Vec::new(),
                event_type: format!("automation_task.{}", task_state.as_str()),
                event_message: format!("automation task is {}", task_state.as_str()),
                event_data: None,
            },
        )
        .await
        .unwrap()
        .unwrap();
}
