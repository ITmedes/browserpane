use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordingMode {
    #[default]
    Disabled,
    Manual,
    Always,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordingFormat {
    #[default]
    Webm,
}

impl SessionRecordingFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webm => "webm",
        }
    }
}

impl FromStr for SessionRecordingFormat {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "webm" => Ok(Self::Webm),
            _ => Err("unknown session recording format"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionRecordingPolicy {
    #[serde(default)]
    pub mode: SessionRecordingMode,
    #[serde(default)]
    pub format: SessionRecordingFormat,
    #[serde(default)]
    pub retention_sec: Option<u32>,
}

impl Default for SessionRecordingPolicy {
    fn default() -> Self {
        Self {
            mode: SessionRecordingMode::Disabled,
            format: SessionRecordingFormat::Webm,
            retention_sec: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordingState {
    Starting,
    Recording,
    Finalizing,
    Ready,
    Failed,
}

impl SessionRecordingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Recording => "recording",
            Self::Finalizing => "finalizing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    pub fn is_active(self) -> bool {
        matches!(self, Self::Starting | Self::Recording | Self::Finalizing)
    }

    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Ready | Self::Failed)
    }
}

impl FromStr for SessionRecordingState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "starting" => Ok(Self::Starting),
            "recording" => Ok(Self::Recording),
            "finalizing" => Ok(Self::Finalizing),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            _ => Err("unknown session recording state"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionRecordingTerminationReason {
    ManualStop,
    SessionStop,
    SessionKill,
    IdleStop,
    GatewayRestart,
    WorkerExit,
}

impl SessionRecordingTerminationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ManualStop => "manual_stop",
            Self::SessionStop => "session_stop",
            Self::SessionKill => "session_kill",
            Self::IdleStop => "idle_stop",
            Self::GatewayRestart => "gateway_restart",
            Self::WorkerExit => "worker_exit",
        }
    }
}

impl FromStr for SessionRecordingTerminationReason {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "manual_stop" => Ok(Self::ManualStop),
            "session_stop" => Ok(Self::SessionStop),
            "session_kill" => Ok(Self::SessionKill),
            "idle_stop" => Ok(Self::IdleStop),
            "gateway_restart" => Ok(Self::GatewayRestart),
            "worker_exit" => Ok(Self::WorkerExit),
            _ => Err("unknown session recording termination reason"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionRecordingResource {
    pub id: Uuid,
    pub session_id: Uuid,
    pub previous_recording_id: Option<Uuid>,
    pub state: SessionRecordingState,
    pub format: SessionRecordingFormat,
    pub mime_type: Option<String>,
    pub bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub termination_reason: Option<SessionRecordingTerminationReason>,
    pub artifact_available: bool,
    pub content_path: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SessionRecordingListResponse {
    pub recordings: Vec<SessionRecordingResource>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CompleteSessionRecordingRequest {
    #[serde(alias = "artifact_path")]
    pub source_path: String,
    #[serde(default)]
    pub mime_type: Option<String>,
    #[serde(default)]
    pub bytes: Option<u64>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct PersistCompletedSessionRecordingRequest {
    pub artifact_ref: String,
    pub mime_type: Option<String>,
    pub bytes: Option<u64>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FailSessionRecordingRequest {
    pub error: String,
    #[serde(default)]
    pub termination_reason: Option<SessionRecordingTerminationReason>,
}

#[derive(Debug, Clone)]
pub struct StoredSessionRecording {
    pub id: Uuid,
    pub session_id: Uuid,
    pub previous_recording_id: Option<Uuid>,
    pub state: SessionRecordingState,
    pub format: SessionRecordingFormat,
    pub mime_type: Option<String>,
    pub bytes: Option<u64>,
    pub duration_ms: Option<u64>,
    pub error: Option<String>,
    pub termination_reason: Option<SessionRecordingTerminationReason>,
    pub artifact_ref: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordingArtifactRetentionCandidate {
    pub session_id: Uuid,
    pub recording_id: Uuid,
    pub artifact_ref: String,
    pub expires_at: DateTime<Utc>,
}

impl StoredSessionRecording {
    pub fn to_resource(&self) -> SessionRecordingResource {
        SessionRecordingResource {
            id: self.id,
            session_id: self.session_id,
            previous_recording_id: self.previous_recording_id,
            state: self.state,
            format: self.format,
            mime_type: self.mime_type.clone(),
            bytes: self.bytes,
            duration_ms: self.duration_ms,
            error: self.error.clone(),
            termination_reason: self.termination_reason,
            artifact_available: self.artifact_ref.is_some(),
            content_path: format!(
                "/api/v1/sessions/{}/recordings/{}/content",
                self.session_id, self.id
            ),
            started_at: self.started_at,
            completed_at: self.completed_at,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
