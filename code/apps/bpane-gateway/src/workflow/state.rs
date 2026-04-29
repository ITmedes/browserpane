use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::automation_tasks::AutomationTaskState;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunState {
    Pending,
    Queued,
    Starting,
    Running,
    AwaitingInput,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

impl From<AutomationTaskState> for WorkflowRunState {
    fn from(value: AutomationTaskState) -> Self {
        match value {
            AutomationTaskState::Pending => Self::Pending,
            AutomationTaskState::Queued => Self::Queued,
            AutomationTaskState::Starting => Self::Starting,
            AutomationTaskState::Running => Self::Running,
            AutomationTaskState::AwaitingInput => Self::AwaitingInput,
            AutomationTaskState::Succeeded => Self::Succeeded,
            AutomationTaskState::Failed => Self::Failed,
            AutomationTaskState::Cancelled => Self::Cancelled,
            AutomationTaskState::TimedOut => Self::TimedOut,
        }
    }
}

impl From<WorkflowRunState> for AutomationTaskState {
    fn from(value: WorkflowRunState) -> Self {
        match value {
            WorkflowRunState::Pending => Self::Pending,
            WorkflowRunState::Queued => Self::Queued,
            WorkflowRunState::Starting => Self::Starting,
            WorkflowRunState::Running => Self::Running,
            WorkflowRunState::AwaitingInput => Self::AwaitingInput,
            WorkflowRunState::Succeeded => Self::Succeeded,
            WorkflowRunState::Failed => Self::Failed,
            WorkflowRunState::Cancelled => Self::Cancelled,
            WorkflowRunState::TimedOut => Self::TimedOut,
        }
    }
}

impl FromStr for WorkflowRunState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "queued" => Ok(Self::Queued),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "awaiting_input" => Ok(Self::AwaitingInput),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "cancelled" => Ok(Self::Cancelled),
            "timed_out" => Ok(Self::TimedOut),
            _ => Err("unknown workflow run state"),
        }
    }
}

impl WorkflowRunState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Queued => "queued",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::AwaitingInput => "awaiting_input",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::TimedOut => "timed_out",
        }
    }

    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Cancelled | Self::TimedOut
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunEventSource {
    Run,
    AutomationTask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunLogSource {
    Run,
    AutomationTask,
}

pub fn workflow_run_event_type(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Pending => "workflow_run.pending",
        WorkflowRunState::Queued => "workflow_run.queued",
        WorkflowRunState::Starting => "workflow_run.starting",
        WorkflowRunState::Running => "workflow_run.running",
        WorkflowRunState::AwaitingInput => "workflow_run.awaiting_input",
        WorkflowRunState::Succeeded => "workflow_run.succeeded",
        WorkflowRunState::Failed => "workflow_run.failed",
        WorkflowRunState::Cancelled => "workflow_run.cancelled",
        WorkflowRunState::TimedOut => "workflow_run.timed_out",
    }
}

pub fn workflow_run_default_message(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Pending => "workflow run returned to pending state",
        WorkflowRunState::Queued => "workflow run queued until worker capacity is available",
        WorkflowRunState::Starting => "workflow run started",
        WorkflowRunState::Running => "workflow run entered running state",
        WorkflowRunState::AwaitingInput => "workflow run is awaiting input",
        WorkflowRunState::Succeeded => "workflow run completed successfully",
        WorkflowRunState::Failed => "workflow run failed",
        WorkflowRunState::Cancelled => "workflow run cancelled",
        WorkflowRunState::TimedOut => "workflow run timed out",
    }
}

pub fn automation_task_event_type_for_run_state(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Pending => "automation_task.pending",
        WorkflowRunState::Queued => "automation_task.queued",
        WorkflowRunState::Starting => "automation_task.starting",
        WorkflowRunState::Running => "automation_task.running",
        WorkflowRunState::AwaitingInput => "automation_task.awaiting_input",
        WorkflowRunState::Succeeded => "automation_task.succeeded",
        WorkflowRunState::Failed => "automation_task.failed",
        WorkflowRunState::Cancelled => "automation_task.cancelled",
        WorkflowRunState::TimedOut => "automation_task.timed_out",
    }
}

pub fn automation_task_default_message_for_run_state(state: WorkflowRunState) -> &'static str {
    match state {
        WorkflowRunState::Pending => "automation task returned to pending state",
        WorkflowRunState::Queued => "automation task queued until worker capacity is available",
        WorkflowRunState::Starting => "automation task started",
        WorkflowRunState::Running => "automation task entered running state",
        WorkflowRunState::AwaitingInput => "automation task is awaiting input",
        WorkflowRunState::Succeeded => "automation task completed successfully",
        WorkflowRunState::Failed => "automation task failed",
        WorkflowRunState::Cancelled => "automation task cancelled",
        WorkflowRunState::TimedOut => "automation task timed out",
    }
}
