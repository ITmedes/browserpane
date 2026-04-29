use super::*;

#[derive(Debug, Clone, PartialEq)]
pub struct AutomationTaskTransitionPlan {
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
}

#[derive(Debug, Clone, PartialEq)]
pub struct AutomationTaskCancellationPlan {
    pub task_state: AutomationTaskState,
    pub task_cancel_requested_at: Option<DateTime<Utc>>,
    pub task_started_at: Option<DateTime<Utc>>,
    pub task_completed_at: Option<DateTime<Utc>>,
    pub task_updated_at: DateTime<Utc>,
    pub task_event_type: String,
    pub task_event_message: String,
    pub task_event_data: Option<Value>,
    pub task_log_stream: AutomationTaskLogStream,
    pub task_log_message: String,
    pub run_event_type: String,
    pub run_event_message: String,
    pub run_event_data: Option<Value>,
    pub run_log_stream: AutomationTaskLogStream,
    pub run_log_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AutomationTaskTransitionPolicyError {
    TaskAlreadyTerminal {
        task_id: Uuid,
    },
    InvalidTaskTransition {
        task_id: Uuid,
        current: AutomationTaskState,
        next: AutomationTaskState,
    },
}

impl std::fmt::Display for AutomationTaskTransitionPolicyError {
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

pub fn plan_automation_task_transition(
    current_task: &StoredAutomationTask,
    request: &AutomationTaskTransitionRequest,
    now: DateTime<Utc>,
) -> Result<AutomationTaskTransitionPlan, AutomationTaskTransitionPolicyError> {
    if current_task.state.is_terminal() {
        return Err(AutomationTaskTransitionPolicyError::TaskAlreadyTerminal {
            task_id: current_task.id,
        });
    }
    if !current_task.state.can_transition_to(request.state) {
        return Err(AutomationTaskTransitionPolicyError::InvalidTaskTransition {
            task_id: current_task.id,
            current: current_task.state,
            next: request.state,
        });
    }

    let task_started_at = if matches!(
        request.state,
        AutomationTaskState::Starting
            | AutomationTaskState::Running
            | AutomationTaskState::AwaitingInput
    ) {
        current_task.started_at.or(Some(now))
    } else {
        current_task.started_at
    };
    let task_completed_at = if request.state.is_terminal() {
        Some(now)
    } else {
        current_task.completed_at
    };

    Ok(AutomationTaskTransitionPlan {
        task_state: request.state,
        task_output: request.output.clone(),
        task_error: request.error.clone(),
        task_artifact_refs: request.artifact_refs.clone(),
        task_started_at,
        task_completed_at,
        task_updated_at: now,
        task_event_type: request.event_type.clone(),
        task_event_message: request.event_message.clone(),
        task_event_data: request.event_data.clone(),
    })
}

pub fn plan_automation_task_cancellation(
    current_task: &StoredAutomationTask,
    now: DateTime<Utc>,
) -> Result<AutomationTaskCancellationPlan, AutomationTaskTransitionPolicyError> {
    if current_task.state.is_terminal() {
        return Err(AutomationTaskTransitionPolicyError::TaskAlreadyTerminal {
            task_id: current_task.id,
        });
    }

    Ok(AutomationTaskCancellationPlan {
        task_state: AutomationTaskState::Cancelled,
        task_cancel_requested_at: Some(now),
        task_started_at: current_task.started_at,
        task_completed_at: Some(now),
        task_updated_at: now,
        task_event_type: "automation_task.cancelled".to_string(),
        task_event_message: "automation task cancelled".to_string(),
        task_event_data: None,
        task_log_stream: AutomationTaskLogStream::System,
        task_log_message: "automation task cancelled".to_string(),
        run_event_type: "workflow_run.cancelled".to_string(),
        run_event_message: "workflow run cancelled".to_string(),
        run_event_data: None,
        run_log_stream: AutomationTaskLogStream::System,
        run_log_message: "workflow run cancelled".to_string(),
    })
}

#[cfg(test)]
mod tests;
