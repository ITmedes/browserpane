use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::session_control::SessionLifecycleState;

use super::{WorkflowRunEventResource, WorkflowRunState};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunInterventionRequest {
    pub request_id: Uuid,
    pub kind: String,
    pub prompt: Option<String>,
    pub details: Option<Value>,
    pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunInterventionAction {
    SubmitInput,
    Resume,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunInterventionResolution {
    pub request_id: Option<Uuid>,
    pub action: WorkflowRunInterventionAction,
    pub input: Option<Value>,
    pub reason: Option<String>,
    pub actor_subject: String,
    pub actor_issuer: String,
    pub actor_display_name: Option<String>,
    pub details: Option<Value>,
    pub resolved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunInterventionResource {
    pub pending_request: Option<WorkflowRunInterventionRequest>,
    pub last_resolution: Option<WorkflowRunInterventionResolution>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunAdmissionState {
    Queued,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunAdmissionResource {
    pub state: WorkflowRunAdmissionState,
    pub reason: String,
    pub details: Option<Value>,
    pub queued_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunResumeMode {
    LiveRuntime,
    ProfileRestart,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunRuntimeResource {
    pub resume_mode: WorkflowRunResumeMode,
    pub exact_runtime_available: bool,
    pub hold_until: Option<DateTime<Utc>>,
    pub released_at: Option<DateTime<Utc>>,
    pub release_reason: Option<String>,
    pub session_state: Option<SessionLifecycleState>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowRunRuntimeHoldMode {
    Live,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunRuntimeHoldRequest {
    pub mode: WorkflowRunRuntimeHoldMode,
    pub timeout_sec: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunRuntimeRelease {
    pub reason: String,
    pub released_at: DateTime<Utc>,
}

fn parse_intervention_request_value(
    value: &Value,
    fallback_request_id: Uuid,
    fallback_requested_at: DateTime<Utc>,
) -> Option<WorkflowRunInterventionRequest> {
    let object = value.as_object()?;
    let nested = object
        .get("intervention_request")
        .and_then(Value::as_object)
        .unwrap_or(object);
    let kind = nested
        .get("kind")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("generic_input")
        .to_string();
    let prompt = nested
        .get("prompt")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let request_id = nested
        .get("request_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or(fallback_request_id);
    let details = nested
        .get("details")
        .cloned()
        .or_else(|| Some(value.clone()));
    Some(WorkflowRunInterventionRequest {
        request_id,
        kind,
        prompt,
        details,
        requested_at: fallback_requested_at,
    })
}

fn parse_intervention_resolution_value(
    value: &Value,
    fallback_resolved_at: DateTime<Utc>,
) -> Option<WorkflowRunInterventionResolution> {
    let object = value.as_object()?;
    let nested = object
        .get("intervention_resolution")
        .and_then(Value::as_object)
        .unwrap_or(object);
    let action = nested
        .get("action")
        .and_then(Value::as_str)
        .and_then(|value| match value {
            "submit_input" => Some(WorkflowRunInterventionAction::SubmitInput),
            "resume" => Some(WorkflowRunInterventionAction::Resume),
            "reject" => Some(WorkflowRunInterventionAction::Reject),
            _ => None,
        })?;
    let actor_subject = nested
        .get("actor_subject")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let actor_issuer = nested
        .get("actor_issuer")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())?
        .to_string();
    let request_id = nested
        .get("request_id")
        .and_then(Value::as_str)
        .and_then(|value| Uuid::parse_str(value).ok());
    let reason = nested
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    let actor_display_name = nested
        .get("actor_display_name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string);
    Some(WorkflowRunInterventionResolution {
        request_id,
        action,
        input: nested.get("input").cloned(),
        reason,
        actor_subject,
        actor_issuer,
        actor_display_name,
        details: nested.get("details").cloned(),
        resolved_at: fallback_resolved_at,
    })
}

pub fn derive_workflow_run_intervention_resource(
    run_state: WorkflowRunState,
    events: &[WorkflowRunEventResource],
) -> WorkflowRunInterventionResource {
    let pending_request = if run_state == WorkflowRunState::AwaitingInput {
        events
            .iter()
            .rev()
            .find(|event| {
                event.event_type == "workflow_run.awaiting_input"
                    || event.event_type == "automation_task.awaiting_input"
            })
            .and_then(|event| {
                event
                    .data
                    .as_ref()
                    .and_then(|value| {
                        parse_intervention_request_value(value, event.id, event.created_at)
                    })
                    .or_else(|| {
                        Some(WorkflowRunInterventionRequest {
                            request_id: event.id,
                            kind: "generic_input".to_string(),
                            prompt: Some(event.message.clone()),
                            details: event.data.clone(),
                            requested_at: event.created_at,
                        })
                    })
            })
    } else {
        None
    };

    let last_resolution = events.iter().rev().find_map(|event| {
        if !matches!(
            event.event_type.as_str(),
            "workflow_run.input_submitted" | "workflow_run.resumed" | "workflow_run.rejected"
        ) {
            return None;
        }
        event
            .data
            .as_ref()
            .and_then(|value| parse_intervention_resolution_value(value, event.created_at))
    });

    WorkflowRunInterventionResource {
        pending_request,
        last_resolution,
    }
}

pub fn derive_workflow_run_admission_resource(
    run_state: WorkflowRunState,
    events: &[WorkflowRunEventResource],
) -> Option<WorkflowRunAdmissionResource> {
    if run_state != WorkflowRunState::Queued {
        return None;
    }

    let event = events.iter().rev().find(|event| {
        event.event_type == "workflow_run.queued" || event.event_type == "automation_task.queued"
    })?;
    let admission = event
        .data
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|value| value.get("admission"))
        .or(event.data.as_ref());
    let reason = admission
        .and_then(Value::as_object)
        .and_then(|value| value.get("reason"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("workflow_worker_capacity")
        .to_string();
    let details = admission
        .and_then(Value::as_object)
        .and_then(|value| value.get("details"))
        .cloned()
        .or_else(|| event.data.clone());

    Some(WorkflowRunAdmissionResource {
        state: WorkflowRunAdmissionState::Queued,
        reason,
        details,
        queued_at: event.created_at,
    })
}

pub fn parse_workflow_run_runtime_hold_request(
    value: &Value,
) -> Result<Option<WorkflowRunRuntimeHoldRequest>, &'static str> {
    let Some(object) = value.as_object() else {
        return Ok(None);
    };
    let Some(runtime_hold) = object.get("runtime_hold") else {
        return Ok(None);
    };
    let hold_object = runtime_hold
        .as_object()
        .ok_or("workflow runtime_hold must be a JSON object")?;
    let mode = hold_object
        .get("mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("live");
    let mode = match mode {
        "live" => WorkflowRunRuntimeHoldMode::Live,
        _ => return Err("workflow runtime_hold.mode must currently be \"live\""),
    };
    let timeout_sec = hold_object
        .get("timeout_sec")
        .and_then(Value::as_u64)
        .ok_or("workflow runtime_hold.timeout_sec must be a positive integer")?;
    if timeout_sec == 0 {
        return Err("workflow runtime_hold.timeout_sec must be greater than zero");
    }
    Ok(Some(WorkflowRunRuntimeHoldRequest { mode, timeout_sec }))
}

fn parse_workflow_run_runtime_release(
    value: &Value,
    fallback_released_at: DateTime<Utc>,
) -> Option<WorkflowRunRuntimeRelease> {
    let object = value.as_object()?;
    let nested = object
        .get("runtime_release")
        .and_then(Value::as_object)
        .unwrap_or(object);
    let reason = nested
        .get("reason")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("runtime_released")
        .to_string();
    Some(WorkflowRunRuntimeRelease {
        reason,
        released_at: fallback_released_at,
    })
}

fn latest_awaiting_input_event(
    events: &[WorkflowRunEventResource],
) -> Option<&WorkflowRunEventResource> {
    events.iter().rev().find(|event| {
        event.event_type == "workflow_run.awaiting_input"
            || event.event_type == "automation_task.awaiting_input"
    })
}

pub fn derive_workflow_run_runtime_resource(
    run_state: WorkflowRunState,
    session_state: Option<SessionLifecycleState>,
    events: &[WorkflowRunEventResource],
) -> Option<WorkflowRunRuntimeResource> {
    let awaiting_input = latest_awaiting_input_event(events)?;
    if run_state != WorkflowRunState::AwaitingInput
        && !events.iter().rev().any(|event| {
            event.created_at >= awaiting_input.created_at
                && event.event_type == "workflow_run.runtime_released"
        })
    {
        return None;
    }

    let hold_request = awaiting_input
        .data
        .as_ref()
        .and_then(|value| parse_workflow_run_runtime_hold_request(value).ok())
        .flatten();
    let hold_until = hold_request.as_ref().and_then(|request| {
        chrono::Duration::from_std(std::time::Duration::from_secs(request.timeout_sec))
            .ok()
            .map(|duration| awaiting_input.created_at + duration)
    });
    let released = events.iter().rev().find_map(|event| {
        if event.created_at < awaiting_input.created_at
            || event.event_type != "workflow_run.runtime_released"
        {
            return None;
        }
        event
            .data
            .as_ref()
            .and_then(|value| parse_workflow_run_runtime_release(value, event.created_at))
            .or_else(|| {
                Some(WorkflowRunRuntimeRelease {
                    reason: "runtime_released".to_string(),
                    released_at: event.created_at,
                })
            })
    });
    let exact_runtime_available = released.is_none()
        && session_state
            .map(SessionLifecycleState::is_runtime_candidate)
            .unwrap_or(false);
    Some(WorkflowRunRuntimeResource {
        resume_mode: if exact_runtime_available {
            WorkflowRunResumeMode::LiveRuntime
        } else {
            WorkflowRunResumeMode::ProfileRestart
        },
        exact_runtime_available,
        hold_until,
        released_at: released.as_ref().map(|value| value.released_at),
        release_reason: released.map(|value| value.reason),
        session_state,
    })
}
