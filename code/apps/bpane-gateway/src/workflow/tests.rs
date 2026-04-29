use chrono::Utc;
use uuid::Uuid;

use super::runtime::{
    WorkflowRunAdmissionState, WorkflowRunInterventionAction, WorkflowRunResumeMode,
    WorkflowRunRuntimeHoldMode,
};
use super::*;
use crate::session_control::SessionLifecycleState;

#[test]
fn derives_pending_intervention_request_from_awaiting_input_event() {
    let request_id = Uuid::now_v7();
    let event = WorkflowRunEventResource {
        id: Uuid::now_v7(),
        run_id: Uuid::now_v7(),
        source: WorkflowRunEventSource::Run,
        automation_task_id: None,
        event_type: "workflow_run.awaiting_input".to_string(),
        message: "workflow run is awaiting input".to_string(),
        data: Some(serde_json::json!({
            "intervention_request": {
                "request_id": request_id,
                "kind": "approval",
                "prompt": "Approve payout export",
                "details": {
                    "task": "review"
                }
            }
        })),
        created_at: Utc::now(),
    };

    let resource =
        derive_workflow_run_intervention_resource(WorkflowRunState::AwaitingInput, &[event]);
    let pending = resource.pending_request.expect("pending request");
    assert_eq!(pending.request_id, request_id);
    assert_eq!(pending.kind, "approval");
    assert_eq!(pending.prompt.as_deref(), Some("Approve payout export"));
    assert_eq!(
        pending.details,
        Some(serde_json::json!({ "task": "review" }))
    );
    assert!(resource.last_resolution.is_none());
}

#[test]
fn derives_last_intervention_resolution_from_resolution_event() {
    let request_id = Uuid::now_v7();
    let event = WorkflowRunEventResource {
        id: Uuid::now_v7(),
        run_id: Uuid::now_v7(),
        source: WorkflowRunEventSource::Run,
        automation_task_id: None,
        event_type: "workflow_run.input_submitted".to_string(),
        message: "operator submitted input".to_string(),
        data: Some(serde_json::json!({
            "intervention_resolution": {
                "request_id": request_id,
                "action": "submit_input",
                "input": {
                    "approved": true
                },
                "actor_subject": "owner",
                "actor_issuer": "bpane-gateway",
                "actor_display_name": "Owner"
            }
        })),
        created_at: Utc::now(),
    };

    let resource = derive_workflow_run_intervention_resource(WorkflowRunState::Running, &[event]);
    assert!(resource.pending_request.is_none());
    let resolution = resource.last_resolution.expect("resolution");
    assert_eq!(resolution.request_id, Some(request_id));
    assert_eq!(
        resolution.action,
        WorkflowRunInterventionAction::SubmitInput
    );
    assert_eq!(
        resolution.input,
        Some(serde_json::json!({ "approved": true }))
    );
    assert_eq!(resolution.actor_subject, "owner");
    assert_eq!(resolution.actor_issuer, "bpane-gateway");
    assert_eq!(resolution.actor_display_name.as_deref(), Some("Owner"));
}

#[test]
fn derives_queued_admission_from_latest_queue_event() {
    let queued_at = Utc::now();
    let resource = derive_workflow_run_admission_resource(
        WorkflowRunState::Queued,
        &[WorkflowRunEventResource {
            id: Uuid::now_v7(),
            run_id: Uuid::now_v7(),
            source: WorkflowRunEventSource::Run,
            automation_task_id: None,
            event_type: "workflow_run.queued".to_string(),
            message: "workflow run queued until worker capacity is available".to_string(),
            data: Some(serde_json::json!({
                "admission": {
                    "reason": "workflow_worker_capacity",
                    "details": {
                        "active_workers": 1,
                        "max_active_workers": 1,
                    }
                }
            })),
            created_at: queued_at,
        }],
    )
    .expect("queued admission");

    assert_eq!(resource.state, WorkflowRunAdmissionState::Queued);
    assert_eq!(resource.reason, "workflow_worker_capacity");
    assert_eq!(resource.queued_at, queued_at);
    assert_eq!(
        resource.details,
        Some(serde_json::json!({
            "active_workers": 1,
            "max_active_workers": 1,
        }))
    );
}

#[test]
fn parses_live_runtime_hold_request() {
    let request = parse_workflow_run_runtime_hold_request(&serde_json::json!({
        "runtime_hold": {
            "mode": "live",
            "timeout_sec": 120
        }
    }))
    .expect("runtime hold request should parse")
    .expect("runtime hold request should be present");

    assert_eq!(request.mode, WorkflowRunRuntimeHoldMode::Live);
    assert_eq!(request.timeout_sec, 120);
}

#[test]
fn derives_live_runtime_resource_for_awaiting_input_run() {
    let awaiting_at = Utc::now();
    let resource = derive_workflow_run_runtime_resource(
        WorkflowRunState::AwaitingInput,
        Some(SessionLifecycleState::Ready),
        &[WorkflowRunEventResource {
            id: Uuid::now_v7(),
            run_id: Uuid::now_v7(),
            source: WorkflowRunEventSource::Run,
            automation_task_id: None,
            event_type: "workflow_run.awaiting_input".to_string(),
            message: "workflow run is awaiting input".to_string(),
            data: Some(serde_json::json!({
                "runtime_hold": {
                    "mode": "live",
                    "timeout_sec": 30
                }
            })),
            created_at: awaiting_at,
        }],
    )
    .expect("runtime resource");

    assert_eq!(resource.resume_mode, WorkflowRunResumeMode::LiveRuntime);
    assert!(resource.exact_runtime_available);
    assert_eq!(resource.session_state, Some(SessionLifecycleState::Ready));
    assert_eq!(
        resource.hold_until,
        Some(awaiting_at + chrono::Duration::seconds(30))
    );
    assert!(resource.released_at.is_none());
    assert!(resource.release_reason.is_none());
}

#[test]
fn derives_profile_restart_runtime_resource_after_release() {
    let awaiting_at = Utc::now();
    let released_at = awaiting_at + chrono::Duration::seconds(45);
    let resource = derive_workflow_run_runtime_resource(
        WorkflowRunState::AwaitingInput,
        Some(SessionLifecycleState::Stopped),
        &[
            WorkflowRunEventResource {
                id: Uuid::now_v7(),
                run_id: Uuid::now_v7(),
                source: WorkflowRunEventSource::Run,
                automation_task_id: None,
                event_type: "workflow_run.awaiting_input".to_string(),
                message: "workflow run is awaiting input".to_string(),
                data: Some(serde_json::json!({
                    "runtime_hold": {
                        "mode": "live",
                        "timeout_sec": 30
                    }
                })),
                created_at: awaiting_at,
            },
            WorkflowRunEventResource {
                id: Uuid::now_v7(),
                run_id: Uuid::now_v7(),
                source: WorkflowRunEventSource::Run,
                automation_task_id: None,
                event_type: "workflow_run.runtime_released".to_string(),
                message: "workflow run released exact live runtime".to_string(),
                data: Some(serde_json::json!({
                    "runtime_release": {
                        "reason": "hold_expired"
                    }
                })),
                created_at: released_at,
            },
        ],
    )
    .expect("runtime resource");

    assert_eq!(resource.resume_mode, WorkflowRunResumeMode::ProfileRestart);
    assert!(!resource.exact_runtime_available);
    assert_eq!(resource.session_state, Some(SessionLifecycleState::Stopped));
    assert_eq!(resource.released_at, Some(released_at));
    assert_eq!(resource.release_reason.as_deref(), Some("hold_expired"));
}
