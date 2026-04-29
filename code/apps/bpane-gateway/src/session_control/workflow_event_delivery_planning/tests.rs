use super::*;

fn stored_run() -> StoredWorkflowRun {
    let now = Utc::now();
    StoredWorkflowRun {
        id: Uuid::now_v7(),
        owner_subject: "owner".to_string(),
        owner_issuer: "https://issuer.example".to_string(),
        workflow_definition_id: Uuid::now_v7(),
        workflow_definition_version_id: Uuid::now_v7(),
        workflow_version: "v1".to_string(),
        session_id: Uuid::now_v7(),
        automation_task_id: Uuid::now_v7(),
        state: WorkflowRunState::Running,
        source_system: None,
        source_reference: None,
        client_request_id: None,
        create_request_fingerprint: None,
        source_snapshot: None,
        extensions: Vec::new(),
        credential_bindings: Vec::new(),
        workspace_inputs: Vec::new(),
        produced_files: Vec::new(),
        input: Some(serde_json::json!({ "step": "login" })),
        output: None,
        error: None,
        artifact_refs: Vec::new(),
        labels: HashMap::new(),
        started_at: Some(now),
        completed_at: None,
        created_at: now,
        updated_at: now,
    }
}

fn stored_event(run_id: Uuid) -> StoredWorkflowRunEvent {
    StoredWorkflowRunEvent {
        id: Uuid::now_v7(),
        run_id,
        event_type: "workflow_run.running".to_string(),
        message: "workflow run entered running state".to_string(),
        data: Some(serde_json::json!({ "phase": "launch" })),
        created_at: Utc::now(),
    }
}

fn subscription(
    owner_subject: &str,
    owner_issuer: &str,
    event_types: Vec<&str>,
) -> StoredWorkflowEventSubscription {
    let now = Utc::now();
    StoredWorkflowEventSubscription {
        id: Uuid::now_v7(),
        owner_subject: owner_subject.to_string(),
        owner_issuer: owner_issuer.to_string(),
        name: "ops".to_string(),
        target_url: "https://example.test/hook".to_string(),
        event_types: event_types.into_iter().map(str::to_string).collect(),
        signing_secret: "secret".to_string(),
        created_at: now,
        updated_at: now,
    }
}

#[test]
fn plans_deliveries_for_matching_owner_and_event_type() {
    let run = stored_run();
    let event = stored_event(run.id);
    let matching = subscription(
        run.owner_subject.as_str(),
        run.owner_issuer.as_str(),
        vec!["workflow_run.*"],
    );
    let foreign_owner = subscription("other", run.owner_issuer.as_str(), vec!["workflow_run.*"]);
    let wrong_event = subscription(
        run.owner_subject.as_str(),
        run.owner_issuer.as_str(),
        vec!["workflow_run.failed"],
    );

    let deliveries = plan_workflow_event_deliveries(
        &[matching.clone(), foreign_owner, wrong_event],
        &run,
        &event,
    );

    assert_eq!(deliveries.len(), 1);
    let delivery = &deliveries[0];
    assert_eq!(delivery.subscription_id, matching.id);
    assert_eq!(delivery.run_id, run.id);
    assert_eq!(delivery.event_id, event.id);
    assert_eq!(delivery.event_type, event.event_type);
    assert_eq!(delivery.target_url, matching.target_url);
    assert_eq!(delivery.signing_secret, matching.signing_secret);
    assert_eq!(delivery.state, WorkflowEventDeliveryState::Pending);
    assert_eq!(delivery.attempt_count, 0);
    assert_eq!(delivery.next_attempt_at, Some(event.created_at));
    assert_eq!(delivery.created_at, event.created_at);
    assert_eq!(delivery.updated_at, event.created_at);
}

#[test]
fn planned_delivery_payload_contains_run_and_event_metadata() {
    let run = stored_run();
    let event = stored_event(run.id);
    let matching = subscription(
        run.owner_subject.as_str(),
        run.owner_issuer.as_str(),
        vec!["workflow_run.running"],
    );

    let delivery = plan_workflow_event_deliveries(std::slice::from_ref(&matching), &run, &event)
        .into_iter()
        .next()
        .unwrap();

    assert_eq!(delivery.payload["subscription_id"], matching.id.to_string());
    assert_eq!(delivery.payload["delivery_id"], delivery.id.to_string());
    assert_eq!(delivery.payload["event_id"], event.id.to_string());
    assert_eq!(delivery.payload["event_type"], event.event_type);
    assert_eq!(delivery.payload["run_id"], run.id.to_string());
    assert_eq!(delivery.payload["workflow_version"], run.workflow_version);
}
