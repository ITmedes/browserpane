use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderValue, Response, StatusCode};
use axum::routing::post;
use axum::Router;
use bpane_runtime_contract::{
    BrokerApiVersion, IdempotencyKey, RuntimeOperation, RuntimeOperationRequest,
    RuntimeOperationResponse, RuntimeOperationResult, SecretValue, StorageHelperAction,
    StorageHelperRequest, RUNTIME_BROKER_API_VERSION_HEADER, RUNTIME_BROKER_PAYLOAD_BYTES_HEADER,
    RUNTIME_BROKER_PAYLOAD_SHA256_HEADER, RUNTIME_BROKER_REQUEST_ID_HEADER,
    RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE, RUNTIME_BROKER_V1_MEDIA_TYPE,
};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use uuid::Uuid;

use crate::{
    AccessTokenProvider, HttpRuntimeBrokerClient, RuntimeBrokerClient, RuntimeBrokerClientConfig,
    RuntimeBrokerClientError, RuntimeBrokerClientErrorCode,
};

struct TokenProvider;

#[async_trait]
impl AccessTokenProvider for TokenProvider {
    async fn access_token(&self) -> Result<SecretValue, RuntimeBrokerClientError> {
        SecretValue::new("service-token")
            .map_err(|_| RuntimeBrokerClientErrorCode::TokenUnavailable.into())
    }
}

fn client(base_url: String, max_payload: usize) -> HttpRuntimeBrokerClient {
    HttpRuntimeBrokerClient::new(
        RuntimeBrokerClientConfig {
            base_url,
            request_timeout: Duration::from_secs(1),
            max_response_bytes: 8192,
            max_storage_payload_bytes: max_payload,
        },
        Arc::new(TokenProvider),
    )
    .unwrap()
}

fn request(action: StorageHelperAction, declared: Option<u64>) -> RuntimeOperationRequest {
    let context_id = Uuid::now_v7();
    RuntimeOperationRequest {
        api_version: BrokerApiVersion::V1,
        request_id: Uuid::now_v7(),
        idempotency_key: IdempotencyKey::new(format!("storage:{action:?}:{}", Uuid::now_v7()))
            .unwrap(),
        operation: RuntimeOperation::RunStorageHelper(StorageHelperRequest {
            action,
            session_id: (action == StorageHelperAction::MaterializeSessionFiles)
                .then_some(context_id),
            source_context_id: action.produces_output_payload().then_some(context_id),
            target_context_id: (action == StorageHelperAction::ImportBrowserContext)
                .then_some(context_id),
            file_target: (action == StorageHelperAction::MaterializeSessionFiles)
                .then_some(bpane_runtime_contract::SessionDataFileTarget::SessionBindingManifest),
            declared_payload_bytes: declared,
        }),
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
async fn sends_authenticated_multipart_input_and_parses_json_result() {
    let operation = request(StorageHelperAction::ImportBrowserContext, Some(7));
    let request_id = operation.request_id;
    let router = Router::new().route(
        "/v1/storage-transfers",
        post(move |request: Request| async move {
            assert_eq!(request.headers()["authorization"], "Bearer service-token");
            assert!(request.headers()["content-type"]
                .to_str()
                .unwrap()
                .starts_with("multipart/form-data; boundary="));
            let body = axum::body::to_bytes(request.into_body(), 16_384)
                .await
                .unwrap();
            assert!(body.windows(7).any(|window| window == b"archive"));
            let response = RuntimeOperationResponse {
                api_version: BrokerApiVersion::V1,
                request_id,
                result: RuntimeOperationResult::Accepted,
            };
            (
                StatusCode::ACCEPTED,
                [("content-type", RUNTIME_BROKER_V1_MEDIA_TYPE)],
                serde_json::to_vec(&response).unwrap(),
            )
        }),
    );
    let base_url = spawn(router).await;

    let response = client(base_url, 1024)
        .execute_storage(&operation, Some(b"archive"))
        .await
        .unwrap();

    assert_eq!(response.response.request_id, request_id);
    assert_eq!(response.response.result, RuntimeOperationResult::Accepted);
    assert!(response.payload.is_none());
}

#[tokio::test]
async fn verifies_binary_export_correlation_length_and_digest() {
    let operation = request(StorageHelperAction::ExportBrowserContext, None);
    let request_id = operation.request_id;
    let payload = Bytes::from_static(b"context-export");
    let digest = hex::encode(Sha256::digest(&payload));
    let response_payload = payload.clone();
    let router = Router::new().route(
        "/v1/storage-transfers",
        post(move || {
            let payload = response_payload.clone();
            let digest = digest.clone();
            async move {
                let mut response = Response::new(Body::from(payload.clone()));
                *response.status_mut() = StatusCode::OK;
                let headers = response.headers_mut();
                headers.insert(
                    "content-type",
                    HeaderValue::from_static(RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE),
                );
                headers.insert(
                    "content-length",
                    HeaderValue::from_str(&payload.len().to_string()).unwrap(),
                );
                headers.insert(
                    RUNTIME_BROKER_API_VERSION_HEADER,
                    HeaderValue::from_static("v1"),
                );
                headers.insert(
                    RUNTIME_BROKER_REQUEST_ID_HEADER,
                    HeaderValue::from_str(&request_id.to_string()).unwrap(),
                );
                headers.insert(
                    RUNTIME_BROKER_PAYLOAD_BYTES_HEADER,
                    HeaderValue::from_str(&payload.len().to_string()).unwrap(),
                );
                headers.insert(
                    RUNTIME_BROKER_PAYLOAD_SHA256_HEADER,
                    HeaderValue::from_str(&digest).unwrap(),
                );
                response
            }
        }),
    );
    let base_url = spawn(router).await;

    let response = client(base_url, 1024)
        .execute_storage(&operation, None)
        .await
        .unwrap();

    assert_eq!(response.payload.as_deref(), Some(payload.as_ref()));
    assert_eq!(
        response.response.result,
        RuntimeOperationResult::StoragePayload {
            payload_bytes: payload.len() as u64,
            sha256_hex: hex::encode(Sha256::digest(&payload)),
        }
    );
}

#[tokio::test]
async fn rejects_invalid_input_before_network_and_invalid_binary_metadata() {
    let unreachable = client("http://127.0.0.1:1".to_string(), 4);
    let import = request(StorageHelperAction::ImportBrowserContext, Some(5));
    for payload in [None, Some(&b"four"[..]), Some(&b"12345"[..])] {
        let error = unreachable
            .execute_storage(&import, payload)
            .await
            .unwrap_err();
        assert_eq!(error.code, RuntimeBrokerClientErrorCode::InvalidRequest);
    }

    let export = request(StorageHelperAction::ExportBrowserContext, None);
    let request_id = export.request_id;
    let router = Router::new().route(
        "/v1/storage-transfers",
        post(move || async move {
            let mut headers = HeaderMap::new();
            headers.insert(
                "content-type",
                HeaderValue::from_static(RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE),
            );
            headers.insert("content-length", HeaderValue::from_static("3"));
            headers.insert(
                RUNTIME_BROKER_API_VERSION_HEADER,
                HeaderValue::from_static("v1"),
            );
            headers.insert(
                RUNTIME_BROKER_REQUEST_ID_HEADER,
                HeaderValue::from_str(&request_id.to_string()).unwrap(),
            );
            headers.insert(
                RUNTIME_BROKER_PAYLOAD_BYTES_HEADER,
                HeaderValue::from_static("3"),
            );
            headers.insert(
                RUNTIME_BROKER_PAYLOAD_SHA256_HEADER,
                HeaderValue::from_str(&"0".repeat(64)).unwrap(),
            );
            (StatusCode::OK, headers, b"bad")
        }),
    );
    let error = client(spawn(router).await, 1024)
        .execute_storage(&export, None)
        .await
        .unwrap_err();
    assert_eq!(error.code, RuntimeBrokerClientErrorCode::InvalidResponse);
}
