use serde_json::Value;

use crate::workflow::{StoredWorkflowRun, WorkflowRunState};

use super::{WorkflowOutcomeCategory, WorkflowRunOutcome, WorkflowSideEffectState};

const MAX_OUTCOME_MESSAGE_LENGTH: usize = 512;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowEndpointRunEvidence {
    pub outcome: WorkflowRunOutcome,
    pub side_effect_state: WorkflowSideEffectState,
}

pub fn derive_terminal_evidence(
    run: &StoredWorkflowRun,
    transition_data: Option<&Value>,
) -> Option<WorkflowEndpointRunEvidence> {
    if run.endpoint_id.is_none() || !run.state.is_terminal() {
        return None;
    }
    let explicit = transition_data.and_then(|data| data.get("workflow_endpoint"));
    let side_effect_state = explicit
        .and_then(|value| value.get("side_effect_state"))
        .and_then(Value::as_str)
        .and_then(|value| value.parse().ok())
        .unwrap_or_else(|| default_side_effect_state(run));
    let mut outcome = explicit
        .and_then(|value| value.get("outcome"))
        .and_then(parse_explicit_outcome)
        .unwrap_or_else(|| default_outcome(run));
    if side_effect_state == WorkflowSideEffectState::Uncertain {
        outcome.retryable = false;
        outcome.retry_after_seconds = None;
    }
    Some(WorkflowEndpointRunEvidence {
        outcome,
        side_effect_state,
    })
}

fn default_side_effect_state(run: &StoredWorkflowRun) -> WorkflowSideEffectState {
    if run.state == WorkflowRunState::Succeeded {
        return WorkflowSideEffectState::Confirmed;
    }
    if run.started_at.is_some() {
        WorkflowSideEffectState::Uncertain
    } else {
        WorkflowSideEffectState::None
    }
}

fn default_outcome(run: &StoredWorkflowRun) -> WorkflowRunOutcome {
    let error = run.error.as_deref().unwrap_or_default();
    let (category, code, message, retryable) = match run.state {
        WorkflowRunState::Succeeded => (
            WorkflowOutcomeCategory::Success,
            "workflow_succeeded",
            "workflow completed successfully",
            false,
        ),
        WorkflowRunState::Cancelled => (
            WorkflowOutcomeCategory::Cancellation,
            "workflow_cancelled",
            "workflow was cancelled",
            false,
        ),
        WorkflowRunState::TimedOut => (
            WorkflowOutcomeCategory::Timeout,
            "workflow_timed_out",
            "workflow execution timed out",
            false,
        ),
        WorkflowRunState::Failed if error.starts_with("workflow_endpoint_output_") => (
            WorkflowOutcomeCategory::ValidationFailure,
            "output_schema_validation_failed",
            "workflow output did not satisfy the endpoint schema",
            false,
        ),
        WorkflowRunState::Failed if error == "external_intervention_required" => (
            WorkflowOutcomeCategory::ExternalInterventionRequired,
            "external_intervention_required",
            "the external process must resolve a challenge or judgment",
            false,
        ),
        WorkflowRunState::Failed if error.starts_with("policy_denied:") => (
            WorkflowOutcomeCategory::PolicyDenial,
            "policy_denied",
            "workflow execution was denied by policy",
            false,
        ),
        WorkflowRunState::Failed if error.starts_with("business_failure:") => (
            WorkflowOutcomeCategory::BusinessFailure,
            "business_failure",
            "workflow reported a business failure",
            false,
        ),
        WorkflowRunState::Failed if error.starts_with("retryable_technical_failure:") => (
            WorkflowOutcomeCategory::RetryableTechnicalFailure,
            "retryable_technical_failure",
            "workflow reported a retryable technical failure",
            true,
        ),
        WorkflowRunState::Failed => (
            WorkflowOutcomeCategory::PermanentTechnicalFailure,
            "workflow_failed",
            "workflow execution failed",
            false,
        ),
        _ => unreachable!("terminal workflow evidence requires a terminal state"),
    };
    WorkflowRunOutcome {
        category,
        code: code.to_string(),
        message: truncate(message),
        details: None,
        retryable,
        retry_after_seconds: None,
        caused_by: Some("workflow_run".to_string()),
    }
}

fn parse_explicit_outcome(value: &Value) -> Option<WorkflowRunOutcome> {
    let mut outcome = serde_json::from_value::<WorkflowRunOutcome>(value.clone()).ok()?;
    if outcome.code.trim().is_empty() || outcome.code.len() > 128 {
        return None;
    }
    outcome.message = truncate(&outcome.message);
    Some(outcome)
}

fn truncate(value: &str) -> String {
    value.chars().take(MAX_OUTCOME_MESSAGE_LENGTH).collect()
}
