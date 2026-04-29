use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunTransitionPlan {
    pub task_state: AutomationTaskState,
    pub task_output: Option<Value>,
    pub task_error: Option<String>,
    pub task_artifact_refs: Vec<String>,
    pub task_started_at: Option<DateTime<Utc>>,
    pub task_completed_at: Option<DateTime<Utc>>,
    pub task_updated_at: DateTime<Utc>,
    pub task_event_type: String,
    pub task_event_message: String,
    pub task_event_data: Option<Value>,
    pub run_state: WorkflowRunState,
    pub run_output: Option<Value>,
    pub run_error: Option<String>,
    pub run_artifact_refs: Vec<String>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub run_completed_at: Option<DateTime<Utc>>,
    pub run_updated_at: DateTime<Utc>,
    pub run_event_type: String,
    pub run_event_message: String,
    pub run_event_data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowRunReconciliationPlan {
    pub run_state: WorkflowRunState,
    pub run_output: Option<Value>,
    pub run_error: Option<String>,
    pub run_artifact_refs: Vec<String>,
    pub run_started_at: Option<DateTime<Utc>>,
    pub run_completed_at: Option<DateTime<Utc>>,
    pub run_updated_at: DateTime<Utc>,
    pub run_event_type: String,
    pub run_event_message: String,
    pub run_event_data: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowRunReconciliationDecision {
    NotTerminal,
    Unchanged,
    Update,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkflowRunTransitionPolicyError {
    TaskAlreadyTerminal {
        task_id: Uuid,
    },
    InvalidTaskTransition {
        task_id: Uuid,
        current: AutomationTaskState,
        next: AutomationTaskState,
    },
}

impl std::fmt::Display for WorkflowRunTransitionPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TaskAlreadyTerminal { task_id } => {
                write!(f, "automation task {} is already terminal", task_id)
            }
            Self::InvalidTaskTransition {
                task_id,
                current,
                next,
            } => write!(
                f,
                "automation task {} cannot transition from {} to {}",
                task_id,
                current.as_str(),
                next.as_str()
            ),
        }
    }
}

pub fn plan_workflow_run_transition(
    current_task: &StoredAutomationTask,
    request: &WorkflowRunTransitionRequest,
    now: DateTime<Utc>,
) -> Result<WorkflowRunTransitionPlan, WorkflowRunTransitionPolicyError> {
    let task_state: AutomationTaskState = request.state.into();
    if current_task.state.is_terminal() {
        return Err(WorkflowRunTransitionPolicyError::TaskAlreadyTerminal {
            task_id: current_task.id,
        });
    }
    if !current_task.state.can_transition_to(task_state) {
        return Err(WorkflowRunTransitionPolicyError::InvalidTaskTransition {
            task_id: current_task.id,
            current: current_task.state,
            next: task_state,
        });
    }

    let task_started_at = if matches!(
        task_state,
        AutomationTaskState::Starting
            | AutomationTaskState::Running
            | AutomationTaskState::AwaitingInput
    ) {
        current_task.started_at.or(Some(now))
    } else {
        current_task.started_at
    };
    let task_completed_at = if task_state.is_terminal() {
        Some(now)
    } else {
        current_task.completed_at
    };
    let task_output = request.output.clone();
    let task_error = request.error.clone();
    let task_artifact_refs = request.artifact_refs.clone();
    let task_event_message = request.message.clone().unwrap_or_else(|| {
        automation_task_default_message_for_run_state(request.state).to_string()
    });
    let run_event_message = request
        .message
        .clone()
        .unwrap_or_else(|| workflow_run_default_message(request.state).to_string());

    Ok(WorkflowRunTransitionPlan {
        task_state,
        task_output: task_output.clone(),
        task_error: task_error.clone(),
        task_artifact_refs: task_artifact_refs.clone(),
        task_started_at,
        task_completed_at,
        task_updated_at: now,
        task_event_type: automation_task_event_type_for_run_state(request.state).to_string(),
        task_event_message,
        task_event_data: request.data.clone(),
        run_state: request.state,
        run_output: task_output,
        run_error: task_error,
        run_artifact_refs: task_artifact_refs,
        run_started_at: task_started_at,
        run_completed_at: task_completed_at,
        run_updated_at: now,
        run_event_type: workflow_run_event_type(request.state).to_string(),
        run_event_message,
        run_event_data: request.data.clone(),
    })
}

pub fn plan_workflow_run_reconciliation(
    current_run: &StoredWorkflowRun,
    current_task: &StoredAutomationTask,
    now: DateTime<Utc>,
) -> (
    WorkflowRunReconciliationDecision,
    Option<WorkflowRunReconciliationPlan>,
) {
    if !current_task.state.is_terminal() {
        return (WorkflowRunReconciliationDecision::NotTerminal, None);
    }

    let run_state: WorkflowRunState = current_task.state.into();
    if current_run.state == run_state
        && current_run.output == current_task.output
        && current_run.error == current_task.error
        && current_run.artifact_refs == current_task.artifact_refs
        && current_run.started_at == current_task.started_at
        && current_run.completed_at == current_task.completed_at
    {
        return (WorkflowRunReconciliationDecision::Unchanged, None);
    }

    (
        WorkflowRunReconciliationDecision::Update,
        Some(WorkflowRunReconciliationPlan {
            run_state,
            run_output: current_task.output.clone(),
            run_error: current_task.error.clone(),
            run_artifact_refs: current_task.artifact_refs.clone(),
            run_started_at: current_task.started_at,
            run_completed_at: current_task.completed_at,
            run_updated_at: now,
            run_event_type: workflow_run_event_type(run_state).to_string(),
            run_event_message: "workflow run reconciled from terminal automation task state"
                .to_string(),
            run_event_data: Some(serde_json::json!({
                "reconciled_from": "automation_task"
            })),
        }),
    )
}

#[cfg(test)]
mod tests;
