use std::collections::HashMap;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialBindingProvider {
    VaultKvV2,
}

impl CredentialBindingProvider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::VaultKvV2 => "vault_kv_v2",
        }
    }
}

impl FromStr for CredentialBindingProvider {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "vault_kv_v2" => Ok(Self::VaultKvV2),
            _ => Err("unknown credential binding provider"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialInjectionMode {
    FormFill,
    CookieSeed,
    StorageSeed,
    TotpFill,
}

impl CredentialInjectionMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::FormFill => "form_fill",
            Self::CookieSeed => "cookie_seed",
            Self::StorageSeed => "storage_seed",
            Self::TotpFill => "totp_fill",
        }
    }
}

impl FromStr for CredentialInjectionMode {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "form_fill" => Ok(Self::FormFill),
            "cookie_seed" => Ok(Self::CookieSeed),
            "storage_seed" => Ok(Self::StorageSeed),
            "totp_fill" => Ok(Self::TotpFill),
            _ => Err("unknown credential injection mode"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialTotpMetadata {
    pub issuer: Option<String>,
    pub account_name: Option<String>,
    pub period_sec: Option<u32>,
    pub digits: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct PersistCredentialBindingRequest {
    pub id: Uuid,
    pub name: String,
    pub provider: CredentialBindingProvider,
    pub external_ref: String,
    pub namespace: Option<String>,
    pub allowed_origins: Vec<String>,
    pub injection_mode: CredentialInjectionMode,
    pub totp: Option<CredentialTotpMetadata>,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct StoredCredentialBinding {
    pub id: Uuid,
    pub owner_subject: String,
    pub owner_issuer: String,
    pub name: String,
    pub provider: CredentialBindingProvider,
    pub external_ref: String,
    pub namespace: Option<String>,
    pub allowed_origins: Vec<String>,
    pub injection_mode: CredentialInjectionMode,
    pub totp: Option<CredentialTotpMetadata>,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CredentialBindingListResponse {
    pub credential_bindings: Vec<CredentialBindingResource>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CredentialBindingResource {
    pub id: Uuid,
    pub name: String,
    pub provider: CredentialBindingProvider,
    pub external_ref: String,
    pub namespace: Option<String>,
    pub allowed_origins: Vec<String>,
    pub injection_mode: CredentialInjectionMode,
    pub totp: Option<CredentialTotpMetadata>,
    pub labels: HashMap<String, String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRunCredentialBinding {
    pub id: Uuid,
    pub name: String,
    pub provider: CredentialBindingProvider,
    pub namespace: Option<String>,
    pub allowed_origins: Vec<String>,
    pub injection_mode: CredentialInjectionMode,
    pub totp: Option<CredentialTotpMetadata>,
    pub external_ref: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorkflowRunCredentialBindingResource {
    pub id: Uuid,
    pub name: String,
    pub provider: CredentialBindingProvider,
    pub namespace: Option<String>,
    pub allowed_origins: Vec<String>,
    pub injection_mode: CredentialInjectionMode,
    pub totp: Option<CredentialTotpMetadata>,
    pub resolve_path: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ResolvedWorkflowRunCredentialBindingResource {
    pub binding: WorkflowRunCredentialBindingResource,
    pub payload: Value,
}

impl StoredCredentialBinding {
    pub fn to_resource(&self) -> CredentialBindingResource {
        CredentialBindingResource {
            id: self.id,
            name: self.name.clone(),
            provider: self.provider,
            external_ref: self.external_ref.clone(),
            namespace: self.namespace.clone(),
            allowed_origins: self.allowed_origins.clone(),
            injection_mode: self.injection_mode,
            totp: self.totp.clone(),
            labels: self.labels.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }

    pub fn to_workflow_run_binding(&self) -> WorkflowRunCredentialBinding {
        WorkflowRunCredentialBinding {
            id: self.id,
            name: self.name.clone(),
            provider: self.provider,
            namespace: self.namespace.clone(),
            allowed_origins: self.allowed_origins.clone(),
            injection_mode: self.injection_mode,
            totp: self.totp.clone(),
            external_ref: self.external_ref.clone(),
        }
    }
}

impl WorkflowRunCredentialBinding {
    pub fn to_resource(&self, run_id: Uuid) -> WorkflowRunCredentialBindingResource {
        WorkflowRunCredentialBindingResource {
            id: self.id,
            name: self.name.clone(),
            provider: self.provider,
            namespace: self.namespace.clone(),
            allowed_origins: self.allowed_origins.clone(),
            injection_mode: self.injection_mode,
            totp: self.totp.clone(),
            resolve_path: format!(
                "/api/v1/workflow-runs/{run_id}/credential-bindings/{}/resolved",
                self.id
            ),
        }
    }
}
