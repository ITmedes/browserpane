use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunLogRetentionCandidate {
    pub run_id: Uuid,
    pub automation_task_id: Uuid,
    pub session_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowRunOutputRetentionCandidate {
    pub run_id: Uuid,
    pub session_id: Uuid,
    pub expires_at: DateTime<Utc>,
}
