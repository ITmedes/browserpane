use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

mod transfer_recorder;

pub(crate) use transfer_recorder::{new_active_transfer_map, SessionFileRecorder};

#[derive(Debug, Clone)]
pub struct PersistSessionFileRequest {
    pub id: Uuid,
    pub session_id: Uuid,
    pub name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub artifact_ref: String,
    pub source: SessionFileSource,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionFileSource {
    BrowserUpload,
    BrowserDownload,
}

impl SessionFileSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BrowserUpload => "browser_upload",
            Self::BrowserDownload => "browser_download",
        }
    }
}

impl FromStr for SessionFileSource {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "browser_upload" => Ok(Self::BrowserUpload),
            "browser_download" => Ok(Self::BrowserDownload),
            _ => Err("unknown session file source"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StoredSessionFile {
    pub id: Uuid,
    pub session_id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub artifact_ref: String,
    pub source: SessionFileSource,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionFileResource {
    pub id: Uuid,
    pub session_id: Uuid,
    pub name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub source: SessionFileSource,
    pub labels: HashMap<String, String>,
    pub content_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SessionFileListResponse {
    pub files: Vec<SessionFileResource>,
}

impl StoredSessionFile {
    pub fn to_resource(&self) -> SessionFileResource {
        SessionFileResource {
            id: self.id,
            session_id: self.session_id,
            name: self.name.clone(),
            media_type: self.media_type.clone(),
            byte_count: self.byte_count,
            sha256_hex: self.sha256_hex.clone(),
            source: self.source,
            labels: self.labels.clone(),
            content_path: format!(
                "/api/v1/sessions/{}/files/{}/content",
                self.session_id, self.id
            ),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PersistSessionFileBindingRequest {
    pub id: Uuid,
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub mount_path: String,
    pub mode: SessionFileBindingMode,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionFileBindingMode {
    ReadOnly,
    ReadWrite,
    ScratchOutput,
}

impl SessionFileBindingMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ReadOnly => "read_only",
            Self::ReadWrite => "read_write",
            Self::ScratchOutput => "scratch_output",
        }
    }
}

impl FromStr for SessionFileBindingMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "read_only" => Ok(Self::ReadOnly),
            "read_write" => Ok(Self::ReadWrite),
            "scratch_output" => Ok(Self::ScratchOutput),
            _ => Err("unknown session file binding mode"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionFileBindingState {
    Pending,
    Materialized,
    Failed,
    Removed,
}

impl FromStr for SessionFileBindingState {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "pending" => Ok(Self::Pending),
            "materialized" => Ok(Self::Materialized),
            "failed" => Ok(Self::Failed),
            "removed" => Ok(Self::Removed),
            _ => Err("unknown session file binding state"),
        }
    }
}

impl SessionFileBindingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Materialized => "materialized",
            Self::Failed => "failed",
            Self::Removed => "removed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct StoredSessionFileBinding {
    pub id: Uuid,
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub artifact_ref: String,
    pub mount_path: String,
    pub mode: SessionFileBindingMode,
    pub state: SessionFileBindingState,
    pub error: Option<String>,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SessionFileBindingResource {
    pub id: Uuid,
    pub session_id: Uuid,
    pub workspace_id: Uuid,
    pub file_id: Uuid,
    pub file_name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub mount_path: String,
    pub mode: SessionFileBindingMode,
    pub state: SessionFileBindingState,
    pub error: Option<String>,
    pub labels: HashMap<String, String>,
    pub content_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct SessionFileBindingListResponse {
    pub bindings: Vec<SessionFileBindingResource>,
}

impl StoredSessionFileBinding {
    pub fn to_resource(&self) -> SessionFileBindingResource {
        SessionFileBindingResource {
            id: self.id,
            session_id: self.session_id,
            workspace_id: self.workspace_id,
            file_id: self.file_id,
            file_name: self.file_name.clone(),
            media_type: self.media_type.clone(),
            byte_count: self.byte_count,
            sha256_hex: self.sha256_hex.clone(),
            provenance: self.provenance.clone(),
            mount_path: self.mount_path.clone(),
            mode: self.mode,
            state: self.state,
            error: self.error.clone(),
            labels: self.labels.clone(),
            content_path: format!(
                "/api/v1/sessions/{}/file-bindings/{}/content",
                self.session_id, self.id
            ),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
