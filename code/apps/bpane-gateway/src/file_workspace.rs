use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PersistFileWorkspaceRequest {
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PersistFileWorkspaceFileRequest {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub artifact_ref: String,
}

#[derive(Debug, Clone)]
pub struct StoredFileWorkspace {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredFileWorkspaceFile {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub artifact_ref: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileWorkspaceResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
    pub files_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct FileWorkspaceListResponse {
    pub workspaces: Vec<FileWorkspaceResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileWorkspaceFileResource {
    pub id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
    pub media_type: Option<String>,
    pub byte_count: u64,
    pub sha256_hex: String,
    pub provenance: Option<Value>,
    pub content_path: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct FileWorkspaceFileListResponse {
    pub files: Vec<FileWorkspaceFileResource>,
}

impl StoredFileWorkspace {
    pub fn to_resource(&self) -> FileWorkspaceResource {
        FileWorkspaceResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            labels: self.labels.clone(),
            files_path: format!("/api/v1/file-workspaces/{}/files", self.id),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl StoredFileWorkspaceFile {
    pub fn to_resource(&self) -> FileWorkspaceFileResource {
        FileWorkspaceFileResource {
            id: self.id,
            workspace_id: self.workspace_id,
            name: self.name.clone(),
            media_type: self.media_type.clone(),
            byte_count: self.byte_count,
            sha256_hex: self.sha256_hex.clone(),
            provenance: self.provenance.clone(),
            content_path: format!(
                "/api/v1/file-workspaces/{}/files/{}/content",
                self.workspace_id, self.id
            ),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}
