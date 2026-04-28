use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

const DEFAULT_VAULT_PREFIX: &str = "browserpane/credential-bindings";

#[derive(Debug, Clone)]
pub struct StoreCredentialSecretRequest {
    pub binding_id: Uuid,
    pub external_ref: Option<String>,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StoredCredentialSecret {
    pub external_ref: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedCredentialSecret {
    pub payload: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum CredentialProviderError {
    #[error("invalid credential provider request: {0}")]
    InvalidRequest(String),
    #[error("credential provider backend failed: {0}")]
    Backend(String),
}

#[async_trait]
pub trait CredentialProviderBackend: Send + Sync {
    async fn store_secret(
        &self,
        request: StoreCredentialSecretRequest,
    ) -> Result<StoredCredentialSecret, CredentialProviderError>;

    async fn resolve_secret(
        &self,
        external_ref: &str,
    ) -> Result<ResolvedCredentialSecret, CredentialProviderError>;
}

#[derive(Clone)]
pub struct CredentialProvider {
    backend: Arc<dyn CredentialProviderBackend>,
}

impl CredentialProvider {
    pub fn new(backend: Arc<dyn CredentialProviderBackend>) -> Self {
        Self { backend }
    }

    pub async fn store_secret(
        &self,
        request: StoreCredentialSecretRequest,
    ) -> Result<StoredCredentialSecret, CredentialProviderError> {
        self.backend.store_secret(request).await
    }

    pub async fn resolve_secret(
        &self,
        external_ref: &str,
    ) -> Result<ResolvedCredentialSecret, CredentialProviderError> {
        self.backend.resolve_secret(external_ref).await
    }
}

#[derive(Debug, Clone)]
pub struct VaultKvV2CredentialProvider {
    client: Client,
    base_url: String,
    token: String,
    mount_path: String,
    key_prefix: String,
}

impl VaultKvV2CredentialProvider {
    pub fn new(
        base_url: String,
        token: String,
        mount_path: String,
        key_prefix: Option<String>,
    ) -> Result<Self, CredentialProviderError> {
        let base_url = base_url.trim().trim_end_matches('/').to_string();
        if base_url.is_empty() {
            return Err(CredentialProviderError::InvalidRequest(
                "vault base url must not be empty".to_string(),
            ));
        }
        let token = token.trim().to_string();
        if token.is_empty() {
            return Err(CredentialProviderError::InvalidRequest(
                "vault token must not be empty".to_string(),
            ));
        }
        let mount_path = normalize_vault_path(&mount_path)?;
        let key_prefix = normalize_vault_path(
            key_prefix
                .as_deref()
                .unwrap_or(DEFAULT_VAULT_PREFIX)
                .trim_matches('/'),
        )?;
        Ok(Self {
            client: Client::new(),
            base_url,
            token,
            mount_path,
            key_prefix,
        })
    }

    fn secret_path(
        &self,
        binding_id: Uuid,
        external_ref: Option<&str>,
    ) -> Result<String, CredentialProviderError> {
        match external_ref {
            Some(value) => normalize_vault_path(value),
            None => Ok(format!("{}/{}", self.key_prefix, binding_id)),
        }
    }

    fn endpoint(&self, external_ref: &str) -> Result<String, CredentialProviderError> {
        let secret_path = normalize_vault_path(external_ref)?;
        Ok(format!(
            "{}/v1/{}/data/{}",
            self.base_url, self.mount_path, secret_path
        ))
    }
}

#[async_trait]
impl CredentialProviderBackend for VaultKvV2CredentialProvider {
    async fn store_secret(
        &self,
        request: StoreCredentialSecretRequest,
    ) -> Result<StoredCredentialSecret, CredentialProviderError> {
        let secret_path = self.secret_path(request.binding_id, request.external_ref.as_deref())?;
        let payload = ensure_json_object(request.payload)?;
        let response = self
            .client
            .post(self.endpoint(&secret_path)?)
            .header("X-Vault-Token", &self.token)
            .json(&serde_json::json!({ "data": payload }))
            .send()
            .await
            .map_err(|error| CredentialProviderError::Backend(error.to_string()))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CredentialProviderError::Backend(format!(
                "vault write failed with status {}{}",
                status,
                if body.is_empty() {
                    String::new()
                } else {
                    format!(": {body}")
                }
            )));
        }
        Ok(StoredCredentialSecret {
            external_ref: secret_path,
        })
    }

    async fn resolve_secret(
        &self,
        external_ref: &str,
    ) -> Result<ResolvedCredentialSecret, CredentialProviderError> {
        let response = self
            .client
            .get(self.endpoint(external_ref)?)
            .header("X-Vault-Token", &self.token)
            .send()
            .await
            .map_err(|error| CredentialProviderError::Backend(error.to_string()))?;
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(CredentialProviderError::Backend(format!(
                "vault read failed with status {}{}",
                status,
                if body.is_empty() {
                    String::new()
                } else {
                    format!(": {body}")
                }
            )));
        }
        let payload = response
            .json::<VaultKvV2ReadResponse>()
            .await
            .map_err(|error| CredentialProviderError::Backend(error.to_string()))?;
        Ok(ResolvedCredentialSecret {
            payload: ensure_json_object(payload.data.data)?,
        })
    }
}

#[derive(Debug, Deserialize)]
struct VaultKvV2ReadResponse {
    data: VaultKvV2ReadResponseData,
}

#[derive(Debug, Deserialize)]
struct VaultKvV2ReadResponseData {
    data: Value,
}

fn normalize_vault_path(path: &str) -> Result<String, CredentialProviderError> {
    let normalized = path
        .trim()
        .trim_matches('/')
        .split('/')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if normalized.is_empty() {
        return Err(CredentialProviderError::InvalidRequest(
            "vault path must not be empty".to_string(),
        ));
    }
    for segment in &normalized {
        if *segment == "." || *segment == ".." {
            return Err(CredentialProviderError::InvalidRequest(
                "vault path must not contain relative traversal".to_string(),
            ));
        }
    }
    Ok(normalized.join("/"))
}

fn ensure_json_object(payload: Value) -> Result<Value, CredentialProviderError> {
    if payload.is_object() {
        Ok(payload)
    } else {
        Err(CredentialProviderError::InvalidRequest(
            "credential payload must be a JSON object".to_string(),
        ))
    }
}
