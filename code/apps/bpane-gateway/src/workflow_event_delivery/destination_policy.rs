use std::collections::HashSet;
use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use ip_network::{Ipv4Network, Ipv6Network};
use reqwest::redirect::Policy as RedirectPolicy;
use reqwest::{Client, Url};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum WorkflowEventDestinationPolicyError {
    #[error("workflow event target_url must be a valid absolute URL")]
    InvalidUrl,
    #[error("workflow event target_url must use http or https")]
    UnsupportedScheme,
    #[error("workflow event target_url must include a host")]
    MissingHost,
    #[error("workflow event target_url must not include credentials")]
    CredentialsNotAllowed,
    #[error("workflow event target_url must not include a fragment")]
    FragmentNotAllowed,
    #[error("workflow event target_url must use https unless its exact origin is allowed")]
    InsecureSchemeNotAllowed,
    #[error("workflow event target_url could not be resolved")]
    ResolutionFailed,
    #[error("workflow event target_url resolved without usable addresses")]
    EmptyResolution,
    #[error("workflow event target_url resolves to a non-public address")]
    NonPublicAddress,
    #[error("workflow event allowed origin must contain only scheme, host, and optional port")]
    InvalidAllowedOrigin,
    #[error("failed to build the workflow event delivery HTTP client")]
    ClientBuild,
}

#[async_trait]
trait WorkflowEventDestinationResolver: Send + Sync {
    async fn resolve(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Vec<SocketAddr>, WorkflowEventDestinationPolicyError>;
}

#[derive(Debug, Default)]
struct SystemWorkflowEventDestinationResolver;

#[async_trait]
impl WorkflowEventDestinationResolver for SystemWorkflowEventDestinationResolver {
    async fn resolve(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Vec<SocketAddr>, WorkflowEventDestinationPolicyError> {
        let addresses = tokio::net::lookup_host((host, port))
            .await
            .map_err(|_error: io::Error| WorkflowEventDestinationPolicyError::ResolutionFailed)?
            .collect::<HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        Ok(addresses)
    }
}

#[derive(Clone)]
pub(crate) struct WorkflowEventDestinationPolicy {
    allowed_origins: Arc<HashSet<String>>,
    resolver: Arc<dyn WorkflowEventDestinationResolver>,
    resolution_timeout: Duration,
}

impl Default for WorkflowEventDestinationPolicy {
    fn default() -> Self {
        Self::new(&[]).expect("an empty workflow event origin policy must be valid")
    }
}

impl WorkflowEventDestinationPolicy {
    pub(crate) fn new(
        allowed_origins: &[String],
    ) -> Result<Self, WorkflowEventDestinationPolicyError> {
        Self::with_resolver_and_timeout(
            allowed_origins,
            Arc::new(SystemWorkflowEventDestinationResolver),
            Duration::from_secs(5),
        )
    }

    pub(crate) fn with_resolution_timeout(
        allowed_origins: &[String],
        resolution_timeout: Duration,
    ) -> Result<Self, WorkflowEventDestinationPolicyError> {
        Self::with_resolver_and_timeout(
            allowed_origins,
            Arc::new(SystemWorkflowEventDestinationResolver),
            resolution_timeout,
        )
    }

    #[cfg(test)]
    fn with_resolver(
        allowed_origins: &[String],
        resolver: Arc<dyn WorkflowEventDestinationResolver>,
    ) -> Result<Self, WorkflowEventDestinationPolicyError> {
        Self::with_resolver_and_timeout(allowed_origins, resolver, Duration::from_secs(5))
    }

    fn with_resolver_and_timeout(
        allowed_origins: &[String],
        resolver: Arc<dyn WorkflowEventDestinationResolver>,
        resolution_timeout: Duration,
    ) -> Result<Self, WorkflowEventDestinationPolicyError> {
        let allowed_origins = allowed_origins
            .iter()
            .map(|origin| parse_allowed_origin(origin))
            .collect::<Result<HashSet<_>, _>>()?;
        Ok(Self {
            allowed_origins: Arc::new(allowed_origins),
            resolver,
            resolution_timeout,
        })
    }

    pub(crate) async fn authorize(
        &self,
        target_url: &str,
    ) -> Result<AuthorizedWorkflowEventDestination, WorkflowEventDestinationPolicyError> {
        let url = parse_target_url(target_url)?;
        let origin = url.origin().ascii_serialization();
        let explicitly_allowed = self.allowed_origins.contains(&origin);
        if url.scheme() != "https" && !explicitly_allowed {
            return Err(WorkflowEventDestinationPolicyError::InsecureSchemeNotAllowed);
        }

        let host = url
            .host()
            .ok_or(WorkflowEventDestinationPolicyError::MissingHost)?;
        let port = url
            .port_or_known_default()
            .ok_or(WorkflowEventDestinationPolicyError::MissingHost)?;
        let (dns_name, addresses) = match host {
            url::Host::Domain(domain) => {
                let addresses = tokio::time::timeout(
                    self.resolution_timeout,
                    self.resolver.resolve(domain, port),
                )
                .await
                .map_err(|_error| WorkflowEventDestinationPolicyError::ResolutionFailed)??;
                (Some(domain.to_string()), addresses)
            }
            url::Host::Ipv4(address) => (None, vec![SocketAddr::new(IpAddr::V4(address), port)]),
            url::Host::Ipv6(address) => (None, vec![SocketAddr::new(IpAddr::V6(address), port)]),
        };

        if addresses.is_empty() {
            return Err(WorkflowEventDestinationPolicyError::EmptyResolution);
        }
        if !explicitly_allowed
            && addresses
                .iter()
                .any(|address| !is_public_unicast(address.ip()))
        {
            return Err(WorkflowEventDestinationPolicyError::NonPublicAddress);
        }
        if explicitly_allowed
            && addresses
                .iter()
                .any(|address| !is_usable_unicast(address.ip()))
        {
            return Err(WorkflowEventDestinationPolicyError::NonPublicAddress);
        }

        Ok(AuthorizedWorkflowEventDestination {
            url,
            dns_name,
            addresses,
        })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AuthorizedWorkflowEventDestination {
    url: Url,
    dns_name: Option<String>,
    addresses: Vec<SocketAddr>,
}

impl AuthorizedWorkflowEventDestination {
    pub(crate) fn canonical_url(&self) -> &str {
        self.url.as_str()
    }

    pub(crate) fn build_client(
        &self,
        request_timeout: Duration,
    ) -> Result<Client, WorkflowEventDestinationPolicyError> {
        let mut builder = Client::builder()
            .timeout(request_timeout)
            .redirect(RedirectPolicy::none())
            .referer(false)
            .no_proxy();
        if let Some(dns_name) = &self.dns_name {
            builder = builder.resolve_to_addrs(dns_name, &self.addresses);
        }
        builder
            .build()
            .map_err(|_error| WorkflowEventDestinationPolicyError::ClientBuild)
    }
}

pub(crate) fn parse_target_url(
    target_url: &str,
) -> Result<Url, WorkflowEventDestinationPolicyError> {
    let url =
        Url::parse(target_url).map_err(|_error| WorkflowEventDestinationPolicyError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(WorkflowEventDestinationPolicyError::UnsupportedScheme);
    }
    if url.host().is_none() {
        return Err(WorkflowEventDestinationPolicyError::MissingHost);
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(WorkflowEventDestinationPolicyError::CredentialsNotAllowed);
    }
    if url.fragment().is_some() {
        return Err(WorkflowEventDestinationPolicyError::FragmentNotAllowed);
    }
    Ok(url)
}

fn parse_allowed_origin(value: &str) -> Result<String, WorkflowEventDestinationPolicyError> {
    let url = parse_target_url(value)
        .map_err(|_error| WorkflowEventDestinationPolicyError::InvalidAllowedOrigin)?;
    if !matches!(url.path(), "" | "/") || url.query().is_some() || url.fragment().is_some() {
        return Err(WorkflowEventDestinationPolicyError::InvalidAllowedOrigin);
    }
    Ok(url.origin().ascii_serialization())
}

fn is_public_unicast(address: IpAddr) -> bool {
    match normalize_mapped_address(address) {
        IpAddr::V4(address) => {
            let network = Ipv4Network::from(address);
            network.is_global() && !network.is_multicast()
        }
        IpAddr::V6(address) => {
            let network = Ipv6Network::from(address);
            network.is_global() && !network.is_multicast()
        }
    }
}

fn is_usable_unicast(address: IpAddr) -> bool {
    match normalize_mapped_address(address) {
        IpAddr::V4(address) => {
            !address.is_unspecified() && !address.is_multicast() && address != Ipv4Addr::BROADCAST
        }
        IpAddr::V6(address) => !address.is_unspecified() && !address.is_multicast(),
    }
}

fn normalize_mapped_address(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(address) => address
            .to_ipv4_mapped()
            .map(IpAddr::V4)
            .unwrap_or(IpAddr::V6(address)),
        address => address,
    }
}

#[cfg(test)]
mod tests;
