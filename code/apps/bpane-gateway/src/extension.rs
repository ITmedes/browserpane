use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct PersistExtensionDefinitionRequest {
    pub name: String,
    pub description: Option<String>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct PersistExtensionVersionRequest {
    pub extension_definition_id: Uuid,
    pub version: String,
    pub install_path: String,
}

#[derive(Debug, Clone)]
pub struct StoredExtensionDefinition {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub latest_version: Option<String>,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct StoredExtensionVersion {
    pub id: Uuid,
    pub extension_definition_id: Uuid,
    pub version: String,
    pub install_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ExtensionDefinitionListResponse {
    pub extensions: Vec<ExtensionDefinitionResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExtensionDefinitionResource {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub enabled: bool,
    pub latest_version: Option<String>,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExtensionVersionResource {
    pub id: Uuid,
    pub extension_definition_id: Uuid,
    pub version: String,
    pub install_path: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedExtension {
    pub extension_id: Uuid,
    pub extension_version_id: Uuid,
    pub name: String,
    pub version: String,
    pub install_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AppliedExtensionResource {
    pub extension_id: Uuid,
    pub extension_version_id: Uuid,
    pub name: String,
    pub version: String,
}

impl StoredExtensionDefinition {
    pub fn to_resource(&self) -> ExtensionDefinitionResource {
        ExtensionDefinitionResource {
            id: self.id,
            name: self.name.clone(),
            description: self.description.clone(),
            enabled: self.enabled,
            latest_version: self.latest_version.clone(),
            labels: self.labels.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

impl StoredExtensionVersion {
    pub fn to_resource(&self) -> ExtensionVersionResource {
        ExtensionVersionResource {
            id: self.id,
            extension_definition_id: self.extension_definition_id,
            version: self.version.clone(),
            install_path: self.install_path.clone(),
            created_at: self.created_at,
        }
    }
}

impl AppliedExtension {
    pub fn to_resource(&self) -> AppliedExtensionResource {
        AppliedExtensionResource {
            extension_id: self.extension_id,
            extension_version_id: self.extension_version_id,
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }
}
