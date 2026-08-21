use serde_json::json;

use super::*;
use crate::workflow::{StoredWorkflowRun, WorkflowRunState};

#[test]
fn draft_2020_12_schema_validation_returns_bounded_json_pointers() {
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": { "period": { "type": "string" } },
        "required": ["period"],
        "additionalProperties": false
    });
    validate_endpoint_schema(&schema).unwrap();
    let errors = validate_schema_instance(&schema, &json!({ "unexpected": true })).unwrap_err();
    assert!(!errors.is_empty());
    assert!(errors.len() <= 8);
    assert!(errors.iter().all(|error| error.pointer.len() <= 256));
}

#[test]
fn schema_validation_rejects_other_drafts() {
    let errors = validate_endpoint_schema(&json!({
        "$schema": "http://json-schema.org/draft-07/schema#",
        "type": "object"
    }))
    .unwrap_err();
    assert_eq!(errors[0].pointer, "/$schema");
}

#[test]
fn request_fingerprint_is_stable_for_object_key_order() {
    assert_eq!(
        canonical_request_fingerprint(&json!({ "a": 1, "b": { "c": 2 } })).unwrap(),
        canonical_request_fingerprint(&json!({ "b": { "c": 2 }, "a": 1 })).unwrap()
    );
}

#[test]
fn uncertain_side_effect_disables_retry_guidance() {
    let run = endpoint_run(
        WorkflowRunState::Failed,
        Some("retryable_technical_failure: portal"),
    );
    let evidence = derive_terminal_evidence(
        &run,
        Some(&json!({
            "workflow_endpoint": {
                "side_effect_state": "uncertain",
                "outcome": {
                    "category": "retryable_technical_failure",
                    "code": "portal_unavailable",
                    "message": "portal unavailable",
                    "details": null,
                    "retryable": true,
                    "retry_after_seconds": 30,
                    "caused_by": "target_system"
                }
            }
        })),
    )
    .unwrap();
    assert_eq!(
        evidence.side_effect_state,
        WorkflowSideEffectState::Uncertain
    );
    assert!(!evidence.outcome.retryable);
    assert_eq!(evidence.outcome.retry_after_seconds, None);
}

fn endpoint_run(state: WorkflowRunState, error: Option<&str>) -> StoredWorkflowRun {
    let now = Utc::now();
    StoredWorkflowRun {
        id: Uuid::now_v7(),
        owner_subject: "owner".to_string(),
        owner_issuer: "issuer".to_string(),
        workflow_definition_id: Uuid::now_v7(),
        workflow_definition_version_id: Uuid::now_v7(),
        workflow_version: "v1".to_string(),
        project_id: Some(Uuid::now_v7()),
        session_id: Uuid::now_v7(),
        automation_task_id: Uuid::now_v7(),
        source_system: None,
        source_reference: None,
        client_request_id: None,
        create_request_fingerprint: None,
        endpoint_id: Some(Uuid::now_v7()),
        endpoint_invocation_id: Some(Uuid::now_v7()),
        endpoint_key: Some("report".to_string()),
        caller_service_principal_id: Some(Uuid::now_v7()),
        endpoint_idempotency_key: Some("request-1".to_string()),
        endpoint_request_fingerprint: Some("fingerprint".to_string()),
        execution_deadline_at: Some(now),
        outcome: None,
        side_effect_state: None,
        source_snapshot: None,
        extensions: Vec::new(),
        credential_bindings: Vec::new(),
        workspace_inputs: Vec::new(),
        produced_files: Vec::new(),
        state,
        input: None,
        output: None,
        error: error.map(ToString::to_string),
        artifact_refs: Vec::new(),
        labels: HashMap::new(),
        started_at: Some(now),
        completed_at: Some(now),
        created_at: now,
        updated_at: now,
    }
}
