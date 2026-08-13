use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;
use tokio::net::TcpListener;

use super::*;

fn config(token_url: String) -> Oauth2ClientCredentialsConfig {
    Oauth2ClientCredentialsConfig {
        token_url,
        client_id: "bpane-runtime-broker-gateway".to_string(),
        client_secret: SecretValue::new("client-secret-never-log").unwrap(),
        scopes: vec!["runtime:launch".to_string()],
        request_timeout: Duration::from_secs(1),
    }
}

async fn spawn(router: Router) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    format!("http://{address}")
}

#[tokio::test]
async fn obtains_and_caches_client_credentials_token() {
    async fn token(
        State(requests): State<Arc<AtomicUsize>>,
        headers: HeaderMap,
        body: String,
    ) -> impl IntoResponse {
        requests.fetch_add(1, Ordering::SeqCst);
        assert!(headers[header::AUTHORIZATION]
            .to_str()
            .unwrap()
            .starts_with("Basic "));
        assert!(body.contains("grant_type=client_credentials"));
        assert!(body.contains("scope=runtime%3Alaunch"));
        Json(json!({
            "access_token": "broker-service-token",
            "token_type": "Bearer",
            "expires_in": 120
        }))
    }
    let requests = Arc::new(AtomicUsize::new(0));
    let base_url = spawn(
        Router::new()
            .route("/token", post(token))
            .with_state(Arc::clone(&requests)),
    )
    .await;
    let provider =
        Oauth2ClientCredentialsProvider::new(config(format!("{base_url}/token"))).unwrap();

    let first = provider.access_token().await.unwrap();
    let second = provider.access_token().await.unwrap();

    assert_eq!(first.expose_secret(), "broker-service-token");
    assert_eq!(second.expose_secret(), "broker-service-token");
    assert_eq!(requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn rejects_redirects_and_unbounded_token_lifetime() {
    let redirect = Router::new()
        .route(
            "/token",
            post(|| async { (StatusCode::FOUND, [(header::LOCATION, "/real")]) }),
        )
        .route(
            "/real",
            get(|| async {
                Json(json!({
                    "access_token": "redirected-token",
                    "token_type": "Bearer",
                    "expires_in": 120
                }))
            }),
        );
    let base_url = spawn(redirect).await;
    let provider =
        Oauth2ClientCredentialsProvider::new(config(format!("{base_url}/token"))).unwrap();
    assert_eq!(
        provider.access_token().await.unwrap_err().code,
        RuntimeBrokerClientErrorCode::TokenUnavailable
    );

    let unbounded = Router::new().route(
        "/token",
        post(|| async {
            Json(json!({
                "access_token": "long-lived-token",
                "token_type": "Bearer",
                "expires_in": 7200
            }))
        }),
    );
    let base_url = spawn(unbounded).await;
    let provider =
        Oauth2ClientCredentialsProvider::new(config(format!("{base_url}/token"))).unwrap();
    assert_eq!(
        provider.access_token().await.unwrap_err().code,
        RuntimeBrokerClientErrorCode::TokenUnavailable
    );
}

#[test]
fn configuration_and_debug_output_never_expose_client_secret() {
    let secret = "client-secret-never-log";
    let provider =
        Oauth2ClientCredentialsProvider::new(config("https://identity.example/token".to_string()))
            .unwrap();
    let debug = format!("{provider:?}");
    assert!(!debug.contains(secret));
    assert!(debug.contains("[REDACTED]"));

    let invalid = Oauth2ClientCredentialsConfig {
        request_timeout: Duration::ZERO,
        ..config("https://identity.example/token".to_string())
    };
    assert_eq!(
        Oauth2ClientCredentialsProvider::new(invalid)
            .unwrap_err()
            .code,
        RuntimeBrokerClientErrorCode::InvalidConfiguration
    );

    let credential_url = config("https://user:secret@identity.example/token".to_string());
    assert_eq!(
        Oauth2ClientCredentialsProvider::new(credential_url)
            .unwrap_err()
            .code,
        RuntimeBrokerClientErrorCode::InvalidConfiguration
    );
}
