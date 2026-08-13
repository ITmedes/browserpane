use std::sync::Arc;
use std::time::Duration;

use bpane_runtime_client::{
    HttpRuntimeBrokerClient, Oauth2ClientCredentialsConfig, Oauth2ClientCredentialsProvider,
    RuntimeBrokerClient, RuntimeBrokerClientConfig, RuntimeBrokerClientErrorCode,
};
use bpane_runtime_contract::{
    BrokerApiVersion, BrowserEgressObservationMode, BrowserEgressSelection, BrowserGeolocation,
    BrowserNetworkIdentity, BrowserProxySelection, BrowserRuntimeFeatures,
    BrowserRuntimeLaunchRequest, BrowserSessionDataSource, ContainerLifecycleAction,
    ContainerLifecycleRequest, IdempotencyKey, RuntimeOperation, RuntimeOperationKind,
    RuntimeOperationRequest, RuntimeOperationResult, SecretValue,
};
use tokio::time::{sleep, Instant};
use uuid::Uuid;

use super::docker::DockerRuntimeManager;
use super::{DockerRuntimeConfig, RuntimeLease, RuntimeManagerError, RuntimeProfile};
use crate::auth::AuthenticatedPrincipal;
use crate::session_control::{
    EgressProfileState, EgressTrafficObservationMode, StoredEgressProfile, StoredSession,
};

#[derive(Clone)]
pub(super) enum BrowserContainerControl {
    Direct,
    Broker(Arc<dyn RuntimeBrokerClient>),
}

#[derive(Debug, Clone)]
pub struct BrokerRuntimeConfig {
    pub docker: DockerRuntimeConfig,
    pub base_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: SecretValue,
    pub request_timeout: Duration,
    pub max_response_bytes: usize,
}

pub(super) fn build_broker_pool(
    config: BrokerRuntimeConfig,
    profile: RuntimeProfile,
) -> Result<DockerRuntimeManager, RuntimeManagerError> {
    let token_provider = Arc::new(
        Oauth2ClientCredentialsProvider::new(Oauth2ClientCredentialsConfig {
            token_url: config.token_url,
            client_id: config.client_id,
            client_secret: config.client_secret,
            scopes: Vec::new(),
            request_timeout: config.request_timeout,
        })
        .map_err(|error| RuntimeManagerError::InvalidConfiguration(error.to_string()))?,
    );
    let client: Arc<dyn RuntimeBrokerClient> = Arc::new(
        HttpRuntimeBrokerClient::new(
            RuntimeBrokerClientConfig {
                base_url: config.base_url,
                request_timeout: config.request_timeout,
                max_response_bytes: config.max_response_bytes,
                max_storage_payload_bytes: 536_870_912,
            },
            token_provider,
        )
        .map_err(|error| RuntimeManagerError::InvalidConfiguration(error.to_string()))?,
    );
    DockerRuntimeManager::new_with_browser_control(
        config.docker,
        profile,
        BrowserContainerControl::Broker(client),
    )
}

impl std::fmt::Debug for BrowserContainerControl {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => formatter.write_str("BrowserContainerControl::Direct"),
            Self::Broker(_) => formatter.write_str("BrowserContainerControl::Broker([REDACTED])"),
        }
    }
}

impl DockerRuntimeManager {
    pub(super) fn runtime_broker_client(&self) -> Option<Arc<dyn RuntimeBrokerClient>> {
        match &self.browser_control {
            BrowserContainerControl::Direct => None,
            BrowserContainerControl::Broker(client) => Some(Arc::clone(client)),
        }
    }

    pub(super) async fn check_browser_control_readiness(&self) -> Result<(), RuntimeManagerError> {
        let BrowserContainerControl::Broker(client) = &self.browser_control else {
            return Ok(());
        };
        client
            .check_readiness()
            .await
            .map_err(|error| RuntimeManagerError::Unavailable(error.to_string()))
    }

    pub(super) async fn launch_browser_with_broker(
        &self,
        lease: &RuntimeLease,
        session_file_bindings: bool,
    ) -> Result<(), RuntimeManagerError> {
        let BrowserContainerControl::Broker(client) = &self.browser_control else {
            return Err(RuntimeManagerError::InvalidConfiguration(
                "runtime broker launch requires broker container control".to_string(),
            ));
        };
        let request = self
            .browser_launch_request(lease, session_file_bindings)
            .await?;
        let result = execute(client, RuntimeOperation::LaunchBrowser(request), "launch").await?;
        if result == RuntimeOperationResult::Accepted {
            Ok(())
        } else {
            Err(RuntimeManagerError::StartupFailed(
                "runtime broker returned an invalid launch result".to_string(),
            ))
        }
    }

    pub(super) async fn stop_browser_container(
        &self,
        session_id: Uuid,
        container_name: &str,
    ) -> Result<(), RuntimeManagerError> {
        let BrowserContainerControl::Broker(client) = &self.browser_control else {
            return self.stop_container(container_name).await;
        };
        let result = execute(
            client,
            RuntimeOperation::ContainerLifecycle(ContainerLifecycleRequest {
                operation_kind: RuntimeOperationKind::BrowserRuntime,
                resource_id: session_id,
                action: ContainerLifecycleAction::Stop,
            }),
            "stop",
        )
        .await?;
        if !matches!(
            result,
            RuntimeOperationResult::Completed { .. } | RuntimeOperationResult::Absent
        ) {
            return Err(RuntimeManagerError::StartupFailed(
                "runtime broker returned an invalid stop result".to_string(),
            ));
        }
        let deadline = Instant::now() + Duration::from_secs(10);
        while self
            .browser_container_exists(session_id, container_name)
            .await?
        {
            if Instant::now() >= deadline {
                return Err(RuntimeManagerError::StartupFailed(
                    "runtime broker browser container remained after stop".to_string(),
                ));
            }
            sleep(Duration::from_millis(100)).await;
        }
        Ok(())
    }

    pub(super) async fn browser_container_exists(
        &self,
        session_id: Uuid,
        container_name: &str,
    ) -> Result<bool, RuntimeManagerError> {
        let BrowserContainerControl::Broker(client) = &self.browser_control else {
            return self.container_exists(container_name).await;
        };
        match execute(
            client,
            RuntimeOperation::ContainerLifecycle(ContainerLifecycleRequest {
                operation_kind: RuntimeOperationKind::BrowserRuntime,
                resource_id: session_id,
                action: ContainerLifecycleAction::Inspect,
            }),
            "inspect",
        )
        .await?
        {
            RuntimeOperationResult::Exists => Ok(true),
            RuntimeOperationResult::Absent => Ok(false),
            _ => Err(RuntimeManagerError::StartupFailed(
                "runtime broker returned an invalid inspect result".to_string(),
            )),
        }
    }

    async fn browser_launch_request(
        &self,
        lease: &RuntimeLease,
        session_file_bindings: bool,
    ) -> Result<BrowserRuntimeLaunchRequest, RuntimeManagerError> {
        let store = self.session_store().await.ok_or_else(|| {
            RuntimeManagerError::StartupFailed(
                "runtime broker launch requires a session store".to_string(),
            )
        })?;
        let session = store
            .get_session_by_id(lease.session_id)
            .await
            .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?
            .ok_or_else(|| {
                RuntimeManagerError::PersistenceFailed(
                    "session was not found while building broker launch intent".to_string(),
                )
            })?;
        let principal = principal_for_session(&session);
        let egress_profile = match session.network_identity.egress_profile_id {
            Some(profile_id) => Some(
                store
                    .get_egress_profile_for_owner(&principal, profile_id)
                    .await
                    .map_err(|error| RuntimeManagerError::PersistenceFailed(error.to_string()))?
                    .ok_or_else(|| {
                        RuntimeManagerError::StartupFailed(
                            "runtime broker egress profile was not found".to_string(),
                        )
                    })?,
            ),
            None => None,
        };
        if egress_profile
            .as_ref()
            .is_some_and(|profile| profile.state == EgressProfileState::Disabled)
        {
            return Err(RuntimeManagerError::StartupFailed(
                "runtime broker egress profile is disabled".to_string(),
            ));
        }
        let features = browser_features(&session, egress_profile.as_ref(), session_file_bindings)?;
        let request = BrowserRuntimeLaunchRequest {
            session_id: lease.session_id,
            browser_context_id: lease.browser_context_id,
            features,
        };
        Ok(request)
    }
}

pub(in crate::runtime_manager) fn browser_features(
    session: &StoredSession,
    egress_profile: Option<&StoredEgressProfile>,
    session_file_bindings: bool,
) -> Result<BrowserRuntimeFeatures, RuntimeManagerError> {
    Ok(BrowserRuntimeFeatures {
        network_identity: BrowserNetworkIdentity {
            locale: session.network_identity.locale.clone(),
            languages: session.network_identity.languages.clone(),
            timezone: session.network_identity.timezone.clone(),
            geolocation: session
                .network_identity
                .geolocation
                .as_ref()
                .map(|value| {
                    Ok(BrowserGeolocation {
                        latitude_e7: scaled_i32(value.latitude, 10_000_000.0)?,
                        longitude_e7: scaled_i32(value.longitude, 10_000_000.0)?,
                        accuracy_mm: value
                            .accuracy_meters
                            .map(|accuracy| scaled_u32(accuracy, 1_000.0))
                            .transpose()?,
                    })
                })
                .transpose()?,
            user_agent: session.network_identity.user_agent.clone(),
            browser_identity: session.network_identity.browser_identity.clone(),
        },
        egress: egress_profile.map(browser_egress_selection),
        extension_version_ids: session
            .extensions
            .iter()
            .map(|extension| extension.extension_version_id)
            .collect(),
        session_file_bindings,
    })
}

fn browser_egress_selection(profile: &StoredEgressProfile) -> BrowserEgressSelection {
    BrowserEgressSelection {
        profile_id: profile.id,
        proxy: profile.proxy.as_ref().map(|proxy| BrowserProxySelection {
            url: proxy.url.clone(),
            authentication: proxy
                .credential_binding_id
                .map(|_| BrowserSessionDataSource::SessionData),
        }),
        bypass_rules: profile.bypass_rules.clone(),
        observation_mode: match profile.traffic_observation.mode {
            EgressTrafficObservationMode::MetadataOnly => {
                BrowserEgressObservationMode::MetadataOnly
            }
            EgressTrafficObservationMode::TlsIntercept => {
                BrowserEgressObservationMode::TlsIntercept
            }
        },
        custom_ca: profile
            .custom_ca
            .as_ref()
            .map(|_| BrowserSessionDataSource::SessionData),
        sensitive_log_sink_configured: profile
            .traffic_observation
            .sensitive_log_sink_ref
            .as_ref()
            .is_some_and(|value| !value.trim().is_empty()),
    }
}

fn principal_for_session(session: &StoredSession) -> AuthenticatedPrincipal {
    AuthenticatedPrincipal {
        subject: session.owner.subject.clone(),
        issuer: session.owner.issuer.clone(),
        display_name: session.owner.display_name.clone(),
        client_id: None,
        safe_claims: Default::default(),
    }
}

fn scaled_i32(value: f64, scale: f64) -> Result<i32, RuntimeManagerError> {
    let scaled = (value * scale).round();
    if !scaled.is_finite() || scaled < f64::from(i32::MIN) || scaled > f64::from(i32::MAX) {
        return Err(invalid_identity());
    }
    Ok(scaled as i32)
}

fn scaled_u32(value: f64, scale: f64) -> Result<u32, RuntimeManagerError> {
    let scaled = (value * scale).round();
    if !scaled.is_finite() || scaled <= 0.0 || scaled > f64::from(u32::MAX) {
        return Err(invalid_identity());
    }
    Ok(scaled as u32)
}

fn invalid_identity() -> RuntimeManagerError {
    RuntimeManagerError::StartupFailed(
        "runtime broker network identity could not be represented safely".to_string(),
    )
}

async fn execute(
    client: &Arc<dyn RuntimeBrokerClient>,
    operation: RuntimeOperation,
    operation_name: &str,
) -> Result<RuntimeOperationResult, RuntimeManagerError> {
    let request_id = Uuid::now_v7();
    let request = RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id,
        idempotency_key: IdempotencyKey::new(format!(
            "browser:{operation_name}:{}:{request_id}",
            operation.resource_id()
        ))
        .map_err(|_| {
            RuntimeManagerError::InvalidConfiguration(
                "runtime broker idempotency key construction failed".to_string(),
            )
        })?,
        operation,
    };
    client
        .execute(&request)
        .await
        .map(|response| response.result)
        .map_err(|error| match error.code {
            RuntimeBrokerClientErrorCode::Unreachable
            | RuntimeBrokerClientErrorCode::Unavailable
            | RuntimeBrokerClientErrorCode::TokenUnavailable => {
                RuntimeManagerError::Unavailable(error.to_string())
            }
            _ => RuntimeManagerError::StartupFailed(error.to_string()),
        })
}
