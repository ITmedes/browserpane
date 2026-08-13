use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bpane_runtime_contract::{
    RuntimeOperationRequest, RuntimeOperationResponse, RUNTIME_BROKER_V1_MEDIA_TYPE,
};
use reqwest::{Client, StatusCode, Url};

use crate::{AccessTokenProvider, RuntimeBrokerClientError, RuntimeBrokerClientErrorCode};

const MAX_REQUEST_BYTES: usize = 65_536;

/// Bounded broker HTTP client settings.
#[derive(Debug, Clone)]
pub struct RuntimeBrokerClientConfig {
    /// Internal broker base URL without credentials, query, or fragment.
    pub base_url: String,
    /// Full operation request deadline.
    pub request_timeout: Duration,
    /// Maximum accepted JSON response bytes.
    pub max_response_bytes: usize,
}

/// Typed runtime operation client boundary used by the gateway.
#[async_trait]
pub trait RuntimeBrokerClient: Send + Sync {
    /// Submits one typed runtime operation.
    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResponse, RuntimeBrokerClientError>;
}

/// OAuth-authenticated HTTP implementation of the runtime broker client.
pub struct HttpRuntimeBrokerClient {
    operation_url: Url,
    request_timeout: Duration,
    max_response_bytes: usize,
    token_provider: Arc<dyn AccessTokenProvider>,
    client: Client,
}

impl HttpRuntimeBrokerClient {
    /// Creates a redirect-disabled, bounded broker HTTP client.
    ///
    /// # Errors
    ///
    /// Returns a sanitized configuration error for an unsafe URL, zero limits,
    /// or HTTP client construction failure.
    pub fn new(
        config: RuntimeBrokerClientConfig,
        token_provider: Arc<dyn AccessTokenProvider>,
    ) -> Result<Self, RuntimeBrokerClientError> {
        let mut base_url = Url::parse(&config.base_url)
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidConfiguration)?;
        if !matches!(base_url.scheme(), "http" | "https")
            || !base_url.username().is_empty()
            || base_url.password().is_some()
            || !matches!(base_url.path(), "" | "/")
            || base_url.query().is_some()
            || base_url.fragment().is_some()
            || config.request_timeout.is_zero()
            || config.max_response_bytes == 0
            || config.max_response_bytes > 1_048_576
        {
            return Err(RuntimeBrokerClientErrorCode::InvalidConfiguration.into());
        }
        base_url.set_path("/v1/operations");
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(config.request_timeout.min(Duration::from_secs(5)))
            .build()
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidConfiguration)?;
        Ok(Self {
            operation_url: base_url,
            request_timeout: config.request_timeout,
            max_response_bytes: config.max_response_bytes,
            token_provider,
            client,
        })
    }

    async fn response_bytes(
        &self,
        mut response: reqwest::Response,
    ) -> Result<Vec<u8>, RuntimeBrokerClientError> {
        if response
            .content_length()
            .is_some_and(|length| length > self.max_response_bytes as u64)
        {
            return Err(RuntimeBrokerClientErrorCode::ResponseTooLarge.into());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|_| RuntimeBrokerClientErrorCode::Unreachable)?
        {
            if bytes.len().saturating_add(chunk.len()) > self.max_response_bytes {
                return Err(RuntimeBrokerClientErrorCode::ResponseTooLarge.into());
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }
}

#[async_trait]
impl RuntimeBrokerClient for HttpRuntimeBrokerClient {
    async fn execute(
        &self,
        request: &RuntimeOperationRequest,
    ) -> Result<RuntimeOperationResponse, RuntimeBrokerClientError> {
        request
            .validate()
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidRequest)?;
        let body = serde_json::to_vec(request)
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidRequest)?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(RuntimeBrokerClientErrorCode::InvalidRequest.into());
        }
        let token = self.token_provider.access_token().await?;
        let response = self
            .client
            .post(self.operation_url.clone())
            .header(reqwest::header::CONTENT_TYPE, RUNTIME_BROKER_V1_MEDIA_TYPE)
            .header(reqwest::header::ACCEPT, RUNTIME_BROKER_V1_MEDIA_TYPE)
            .bearer_auth(token.expose_secret())
            .timeout(self.request_timeout)
            .body(body)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    RuntimeBrokerClientError::from(RuntimeBrokerClientErrorCode::TimedOut)
                } else {
                    RuntimeBrokerClientError::from(RuntimeBrokerClientErrorCode::Unreachable)
                }
            })?;
        if !response.status().is_success() {
            return Err(map_status(response.status()).into());
        }
        if !has_contract_media_type(response.headers()) {
            return Err(RuntimeBrokerClientErrorCode::InvalidResponse.into());
        }
        let bytes = self.response_bytes(response).await?;
        let response: RuntimeOperationResponse = serde_json::from_slice(&bytes)
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidResponse)?;
        if response.request_id != request.request_id {
            return Err(RuntimeBrokerClientErrorCode::InvalidResponse.into());
        }
        Ok(response)
    }
}

fn has_contract_media_type(headers: &reqwest::header::HeaderMap) -> bool {
    headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .is_some_and(|value| value.trim() == RUNTIME_BROKER_V1_MEDIA_TYPE)
}

fn map_status(status: StatusCode) -> RuntimeBrokerClientErrorCode {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => {
            RuntimeBrokerClientErrorCode::AuthenticationRejected
        }
        StatusCode::CONFLICT => RuntimeBrokerClientErrorCode::Conflict,
        StatusCode::TOO_MANY_REQUESTS => RuntimeBrokerClientErrorCode::Overloaded,
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => {
            RuntimeBrokerClientErrorCode::TimedOut
        }
        StatusCode::SERVICE_UNAVAILABLE | StatusCode::BAD_GATEWAY => {
            RuntimeBrokerClientErrorCode::Unavailable
        }
        _ => RuntimeBrokerClientErrorCode::Rejected,
    }
}

impl std::fmt::Debug for HttpRuntimeBrokerClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("HttpRuntimeBrokerClient")
            .field("operation_url", &self.operation_url)
            .field("request_timeout", &self.request_timeout)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
