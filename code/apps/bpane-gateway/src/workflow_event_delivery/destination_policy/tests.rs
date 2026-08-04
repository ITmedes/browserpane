use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::http::{header, StatusCode};
use axum::response::IntoResponse;
use axum::routing::post;
use tokio::net::TcpListener;

use super::*;

#[derive(Debug, Default)]
struct StaticResolver {
    answers: HashMap<String, Vec<IpAddr>>,
    failures: HashSet<String>,
}

#[derive(Debug)]
struct PendingResolver;

#[async_trait]
impl WorkflowEventDestinationResolver for PendingResolver {
    async fn resolve(
        &self,
        _host: &str,
        _port: u16,
    ) -> Result<Vec<SocketAddr>, WorkflowEventDestinationPolicyError> {
        std::future::pending().await
    }
}

impl StaticResolver {
    fn with_answer(host: &str, addresses: &[&str]) -> Self {
        Self {
            answers: HashMap::from([(
                host.to_string(),
                addresses
                    .iter()
                    .map(|address| address.parse().expect("valid test IP address"))
                    .collect(),
            )]),
            failures: HashSet::new(),
        }
    }
}

#[async_trait]
impl WorkflowEventDestinationResolver for StaticResolver {
    async fn resolve(
        &self,
        host: &str,
        port: u16,
    ) -> Result<Vec<SocketAddr>, WorkflowEventDestinationPolicyError> {
        if self.failures.contains(host) {
            return Err(WorkflowEventDestinationPolicyError::ResolutionFailed);
        }
        Ok(self
            .answers
            .get(host)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|address| SocketAddr::new(address, port))
            .collect())
    }
}

fn policy_with_answer(
    allowed_origins: &[String],
    host: &str,
    addresses: &[&str],
) -> WorkflowEventDestinationPolicy {
    WorkflowEventDestinationPolicy::with_resolver(
        allowed_origins,
        Arc::new(StaticResolver::with_answer(host, addresses)),
    )
    .expect("test destination policy")
}

#[tokio::test]
async fn canonicalizes_public_https_target() {
    let policy = policy_with_answer(&[], "example.com", &["93.184.216.34"]);

    let destination = policy
        .authorize("HTTPS://EXAMPLE.COM:443/events?kind=workflow")
        .await
        .unwrap();

    assert_eq!(
        destination.canonical_url(),
        "https://example.com/events?kind=workflow"
    );
}

#[tokio::test]
async fn rejects_structurally_unsafe_targets() {
    let policy = policy_with_answer(&[], "example.com", &["93.184.216.34"]);
    let cases = [
        ("/events", WorkflowEventDestinationPolicyError::InvalidUrl),
        (
            "ftp://example.com/events",
            WorkflowEventDestinationPolicyError::UnsupportedScheme,
        ),
        (
            "https://user:secret@example.com/events",
            WorkflowEventDestinationPolicyError::CredentialsNotAllowed,
        ),
        (
            "https://example.com/events#fragment",
            WorkflowEventDestinationPolicyError::FragmentNotAllowed,
        ),
        (
            "http://example.com/events",
            WorkflowEventDestinationPolicyError::InsecureSchemeNotAllowed,
        ),
    ];

    for (target, expected) in cases {
        assert_eq!(
            policy.authorize(target).await.unwrap_err(),
            expected,
            "{target}"
        );
    }
}

#[tokio::test]
async fn rejects_non_public_ip_literals_and_ipv4_mapped_ipv6() {
    let policy = WorkflowEventDestinationPolicy::new(&[]).unwrap();
    let targets = [
        "https://0.0.0.0/events",
        "https://10.0.0.1/events",
        "https://100.64.0.1/events",
        "https://127.0.0.1/events",
        "https://127.1/events",
        "https://0177.0.0.1/events",
        "https://0x7f.0.0.1/events",
        "https://2130706433/events",
        "https://169.254.169.254/events",
        "https://172.16.0.1/events",
        "https://192.0.2.1/events",
        "https://192.168.0.1/events",
        "https://198.18.0.1/events",
        "https://224.0.0.1/events",
        "https://255.255.255.255/events",
        "https://[::]/events",
        "https://[::1]/events",
        "https://[fc00::1]/events",
        "https://[fe80::1]/events",
        "https://[ff00::1]/events",
        "https://[2001:db8::1]/events",
        "https://[::ffff:127.0.0.1]/events",
    ];

    for target in targets {
        assert_eq!(
            policy.authorize(target).await.unwrap_err(),
            WorkflowEventDestinationPolicyError::NonPublicAddress,
            "{target}"
        );
    }
}

#[tokio::test]
async fn rejects_mixed_empty_and_failed_dns_answers() {
    let mixed = policy_with_answer(&[], "mixed.example", &["93.184.216.34", "10.0.0.1"]);
    assert_eq!(
        mixed
            .authorize("https://mixed.example/events")
            .await
            .unwrap_err(),
        WorkflowEventDestinationPolicyError::NonPublicAddress
    );

    let empty = policy_with_answer(&[], "empty.example", &[]);
    assert_eq!(
        empty
            .authorize("https://empty.example/events")
            .await
            .unwrap_err(),
        WorkflowEventDestinationPolicyError::EmptyResolution
    );

    let failed = WorkflowEventDestinationPolicy::with_resolver(
        &[],
        Arc::new(StaticResolver {
            answers: HashMap::new(),
            failures: HashSet::from(["failed.example".to_string()]),
        }),
    )
    .unwrap();
    assert_eq!(
        failed
            .authorize("https://failed.example/events")
            .await
            .unwrap_err(),
        WorkflowEventDestinationPolicyError::ResolutionFailed
    );
}

#[tokio::test]
async fn bounds_dns_resolution_time() {
    let policy = WorkflowEventDestinationPolicy::with_resolver_and_timeout(
        &[],
        Arc::new(PendingResolver),
        Duration::from_millis(5),
    )
    .unwrap();

    assert_eq!(
        policy
            .authorize("https://slow.example/events")
            .await
            .unwrap_err(),
        WorkflowEventDestinationPolicyError::ResolutionFailed
    );
}

#[tokio::test]
async fn exact_allowed_origin_permits_controlled_http_and_private_addresses() {
    let allowed_origins = vec!["http://localhost:8080".to_string()];
    let policy = policy_with_answer(&allowed_origins, "localhost", &["127.0.0.1", "::1"]);

    let destination = policy
        .authorize("http://localhost:8080/events?kind=workflow")
        .await
        .unwrap();
    assert_eq!(
        destination.canonical_url(),
        "http://localhost:8080/events?kind=workflow"
    );
    assert_eq!(
        policy
            .authorize("http://localhost:8081/events")
            .await
            .unwrap_err(),
        WorkflowEventDestinationPolicyError::InsecureSchemeNotAllowed
    );
    assert_eq!(
        policy
            .authorize("http://localhost.example:8080/events")
            .await
            .unwrap_err(),
        WorkflowEventDestinationPolicyError::InsecureSchemeNotAllowed
    );
}

#[test]
fn rejects_non_origin_allowlist_entries() {
    for value in [
        "example.com",
        "ftp://example.com",
        "https://user@example.com",
        "https://example.com/events",
        "https://example.com?query=true",
        "https://example.com/#fragment",
    ] {
        assert_eq!(
            WorkflowEventDestinationPolicy::new(&[value.to_string()]).err(),
            Some(WorkflowEventDestinationPolicyError::InvalidAllowedOrigin),
            "{value}"
        );
    }
}

#[tokio::test]
async fn allowed_origin_still_rejects_unspecified_multicast_and_broadcast() {
    for origin in [
        "http://0.0.0.0:8080",
        "http://224.0.0.1:8080",
        "http://255.255.255.255:8080",
        "http://[::]:8080",
        "http://[ff00::1]:8080",
    ] {
        let policy = WorkflowEventDestinationPolicy::new(&[origin.to_string()]).unwrap();
        assert_eq!(
            policy
                .authorize(&format!("{origin}/events"))
                .await
                .unwrap_err(),
            WorkflowEventDestinationPolicyError::NonPublicAddress,
            "{origin}"
        );
    }
}

#[tokio::test]
async fn pins_validated_dns_and_does_not_follow_redirects() {
    let redirected_requests = Arc::new(AtomicUsize::new(0));
    let redirected_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirected_address = redirected_listener.local_addr().unwrap();
    let redirected_counter = redirected_requests.clone();
    let redirected_task = tokio::spawn(async move {
        axum::serve(
            redirected_listener,
            axum::Router::new().route(
                "/events",
                post(move || {
                    let redirected_counter = redirected_counter.clone();
                    async move {
                        redirected_counter.fetch_add(1, Ordering::SeqCst);
                        StatusCode::NO_CONTENT
                    }
                }),
            ),
        )
        .await
        .unwrap();
    });

    let redirect_listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let redirect_address = redirect_listener.local_addr().unwrap();
    let location = format!("http://{redirected_address}/events");
    let redirect_task = tokio::spawn(async move {
        axum::serve(
            redirect_listener,
            axum::Router::new().route(
                "/events",
                post(move || {
                    let location = location.clone();
                    async move { (StatusCode::FOUND, [(header::LOCATION, location)]).into_response() }
                }),
            ),
        )
        .await
        .unwrap();
    });

    let origin = format!("http://receiver.example:{}", redirect_address.port());
    let policy = policy_with_answer(
        std::slice::from_ref(&origin),
        "receiver.example",
        &["127.0.0.1"],
    );
    let destination = policy.authorize(&format!("{origin}/events")).await.unwrap();
    let client = destination.build_client(Duration::from_secs(2)).unwrap();
    let response = client
        .post(destination.canonical_url())
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FOUND);
    assert_eq!(redirected_requests.load(Ordering::SeqCst), 0);
    redirect_task.abort();
    redirected_task.abort();
}
