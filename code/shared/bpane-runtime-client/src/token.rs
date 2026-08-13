use std::time::{Duration, Instant};

use async_trait::async_trait;
use bpane_runtime_contract::SecretValue;
use oauth2::basic::BasicClient;
use oauth2::{ClientId, ClientSecret, Scope, TokenResponse, TokenUrl};
use tokio::sync::Mutex;

use crate::{RuntimeBrokerClientError, RuntimeBrokerClientErrorCode};

const TOKEN_REFRESH_SKEW: Duration = Duration::from_secs(30);
const MAX_TOKEN_LIFETIME: Duration = Duration::from_secs(3_600);

/// OAuth2 client-credentials configuration for the gateway service identity.
#[derive(Debug, Clone)]
pub struct Oauth2ClientCredentialsConfig {
    /// OAuth2 token endpoint.
    pub token_url: String,
    /// Confidential gateway client id.
    pub client_id: String,
    /// Confidential gateway client secret.
    pub client_secret: SecretValue,
    /// Optional scopes requested from the identity provider.
    pub scopes: Vec<String>,
    /// Hard token endpoint request timeout.
    pub request_timeout: Duration,
}

/// Supplies a bounded service bearer credential to the broker client.
#[async_trait]
pub trait AccessTokenProvider: Send + Sync {
    /// Returns a valid service access token, refreshing it when required.
    async fn access_token(&self) -> Result<SecretValue, RuntimeBrokerClientError>;
}

struct CachedToken {
    value: SecretValue,
    expires_at: Instant,
}

/// Standards-based OAuth2 client-credentials token provider with safe caching.
pub struct Oauth2ClientCredentialsProvider {
    token_url: TokenUrl,
    client_id: ClientId,
    client_secret: ClientSecret,
    scopes: Vec<Scope>,
    http_client: oauth2::reqwest::Client,
    cache: Mutex<Option<CachedToken>>,
}

impl Oauth2ClientCredentialsProvider {
    /// Creates a provider using a redirect-disabled async HTTP client.
    ///
    /// # Errors
    ///
    /// Returns a sanitized configuration error for invalid URLs, empty client
    /// ids, empty scopes, or a zero request timeout.
    pub fn new(config: Oauth2ClientCredentialsConfig) -> Result<Self, RuntimeBrokerClientError> {
        if config.client_id.trim().is_empty()
            || config.request_timeout.is_zero()
            || config.scopes.iter().any(|scope| scope.trim().is_empty())
        {
            return Err(RuntimeBrokerClientErrorCode::InvalidConfiguration.into());
        }
        let token_url = TokenUrl::new(config.token_url)
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidConfiguration)?;
        if !matches!(token_url.url().scheme(), "http" | "https")
            || !token_url.url().username().is_empty()
            || token_url.url().password().is_some()
            || token_url.url().query().is_some()
            || token_url.url().fragment().is_some()
        {
            return Err(RuntimeBrokerClientErrorCode::InvalidConfiguration.into());
        }
        let http_client = oauth2::reqwest::ClientBuilder::new()
            .redirect(oauth2::reqwest::redirect::Policy::none())
            .timeout(config.request_timeout)
            .build()
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidConfiguration)?;
        Ok(Self {
            token_url,
            client_id: ClientId::new(config.client_id),
            client_secret: ClientSecret::new(config.client_secret.expose_secret().to_string()),
            scopes: config.scopes.into_iter().map(Scope::new).collect(),
            http_client,
            cache: Mutex::new(None),
        })
    }

    async fn refresh(&self) -> Result<CachedToken, RuntimeBrokerClientError> {
        let client = BasicClient::new(self.client_id.clone())
            .set_client_secret(self.client_secret.clone())
            .set_token_uri(self.token_url.clone());
        let mut request = client.exchange_client_credentials();
        for scope in &self.scopes {
            request = request.add_scope(scope.clone());
        }
        let response = request
            .request_async(&self.http_client)
            .await
            .map_err(|_| RuntimeBrokerClientErrorCode::TokenUnavailable)?;
        let lifetime = response
            .expires_in()
            .filter(|lifetime| !lifetime.is_zero() && *lifetime <= MAX_TOKEN_LIFETIME)
            .ok_or(RuntimeBrokerClientErrorCode::TokenUnavailable)?;
        let value = SecretValue::new(response.access_token().secret().to_string())
            .map_err(|_| RuntimeBrokerClientErrorCode::TokenUnavailable)?;
        Ok(CachedToken {
            value,
            expires_at: Instant::now() + lifetime,
        })
    }
}

#[async_trait]
impl AccessTokenProvider for Oauth2ClientCredentialsProvider {
    async fn access_token(&self) -> Result<SecretValue, RuntimeBrokerClientError> {
        let mut cache = self.cache.lock().await;
        if let Some(token) = cache.as_ref() {
            let refresh_at = token
                .expires_at
                .checked_sub(TOKEN_REFRESH_SKEW)
                .unwrap_or(token.expires_at);
            if Instant::now() < refresh_at {
                return Ok(token.value.clone());
            }
        }
        let token = self.refresh().await?;
        let value = token.value.clone();
        *cache = Some(token);
        Ok(value)
    }
}

impl std::fmt::Debug for Oauth2ClientCredentialsProvider {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Oauth2ClientCredentialsProvider")
            .field("token_url", &self.token_url)
            .field("client_id", &self.client_id)
            .field("client_secret", &"[REDACTED]")
            .field("scopes", &self.scopes)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
