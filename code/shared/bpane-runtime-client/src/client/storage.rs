use bpane_runtime_contract::{
    BrokerApiVersion, RuntimeOperation, RuntimeOperationRequest, RuntimeOperationResponse,
    RuntimeOperationResult, RUNTIME_BROKER_API_VERSION_HEADER, RUNTIME_BROKER_PAYLOAD_BYTES_HEADER,
    RUNTIME_BROKER_PAYLOAD_SHA256_HEADER, RUNTIME_BROKER_REQUEST_ID_HEADER,
    RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE, RUNTIME_BROKER_V1_MEDIA_TYPE,
};
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use reqwest::multipart::{Form, Part};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    has_contract_media_type, map_status, HttpRuntimeBrokerClient, RuntimeStorageOperationResponse,
    MAX_REQUEST_BYTES,
};
use crate::{RuntimeBrokerClientError, RuntimeBrokerClientErrorCode};

impl HttpRuntimeBrokerClient {
    pub(super) async fn execute_storage_http(
        &self,
        request: &RuntimeOperationRequest,
        payload: Option<&[u8]>,
    ) -> Result<RuntimeStorageOperationResponse, RuntimeBrokerClientError> {
        let storage = match &request.operation {
            RuntimeOperation::RunStorageHelper(storage) => storage,
            _ => return Err(RuntimeBrokerClientErrorCode::InvalidRequest.into()),
        };
        request
            .validate()
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidRequest)?;
        validate_input(
            storage.action.accepts_input_payload(),
            storage.declared_payload_bytes,
            payload,
            self.max_storage_payload_bytes,
        )?;
        let metadata = serde_json::to_vec(request)
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidRequest)?;
        if metadata.len() > MAX_REQUEST_BYTES {
            return Err(RuntimeBrokerClientErrorCode::InvalidRequest.into());
        }
        let request_part = Part::bytes(metadata)
            .mime_str(RUNTIME_BROKER_V1_MEDIA_TYPE)
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidRequest)?;
        let mut form = Form::new().part("request", request_part);
        if let Some(payload) = payload {
            let payload_part = Part::bytes(payload.to_vec())
                .mime_str(RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE)
                .map_err(|_| RuntimeBrokerClientErrorCode::InvalidRequest)?;
            form = form.part("payload", payload_part);
        }
        let token = self.token_provider.access_token().await?;
        let response = self
            .with_trace_context(self.client.post(self.storage_transfer_url.clone()))
            .header(
                ACCEPT,
                format!(
                    "{RUNTIME_BROKER_V1_MEDIA_TYPE}, {RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE}"
                ),
            )
            .bearer_auth(token.expose_secret())
            .multipart(form)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(map_send_error)?;
        if !response.status().is_success() {
            return Err(map_status(response.status()).into());
        }
        if has_contract_media_type(response.headers()) {
            return self
                .json_storage_response(request, storage.action.produces_output_payload(), response)
                .await;
        }
        if content_type(response.headers()) != Some(RUNTIME_BROKER_STORAGE_PAYLOAD_MEDIA_TYPE)
            || !storage.action.produces_output_payload()
        {
            return Err(RuntimeBrokerClientErrorCode::InvalidResponse.into());
        }
        self.binary_storage_response(request, response).await
    }
    async fn json_storage_response(
        &self,
        request: &RuntimeOperationRequest,
        produces_payload: bool,
        response: reqwest::Response,
    ) -> Result<RuntimeStorageOperationResponse, RuntimeBrokerClientError> {
        let bytes = self.response_bytes(response).await?;
        let response: RuntimeOperationResponse = serde_json::from_slice(&bytes)
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidResponse)?;
        let valid_result = if produces_payload {
            response.result == RuntimeOperationResult::Absent
        } else {
            !matches!(
                response.result,
                RuntimeOperationResult::StoragePayload { .. }
            )
        };
        if response.request_id != request.request_id || !valid_result {
            return Err(RuntimeBrokerClientErrorCode::InvalidResponse.into());
        }
        Ok(RuntimeStorageOperationResponse {
            response,
            payload: None,
        })
    }

    async fn binary_storage_response(
        &self,
        request: &RuntimeOperationRequest,
        mut response: reqwest::Response,
    ) -> Result<RuntimeStorageOperationResponse, RuntimeBrokerClientError> {
        let request_id = required_header(response.headers(), RUNTIME_BROKER_REQUEST_ID_HEADER)?
            .parse::<Uuid>()
            .map_err(|_| RuntimeBrokerClientErrorCode::InvalidResponse)?;
        let version = required_header(response.headers(), RUNTIME_BROKER_API_VERSION_HEADER)?;
        let payload_bytes =
            required_header(response.headers(), RUNTIME_BROKER_PAYLOAD_BYTES_HEADER)?
                .parse::<u64>()
                .map_err(|_| RuntimeBrokerClientErrorCode::InvalidResponse)?;
        let sha256_hex =
            required_header(response.headers(), RUNTIME_BROKER_PAYLOAD_SHA256_HEADER)?.to_string();
        if request_id != request.request_id
            || version != "v1"
            || payload_bytes == 0
            || payload_bytes > self.max_storage_payload_bytes as u64
            || response.content_length() != Some(payload_bytes)
            || !is_lowercase_sha256(&sha256_hex)
        {
            return Err(RuntimeBrokerClientErrorCode::InvalidResponse.into());
        }
        let capacity = usize::try_from(payload_bytes)
            .map_err(|_| RuntimeBrokerClientErrorCode::ResponseTooLarge)?;
        let mut payload = Vec::with_capacity(capacity);
        while let Some(chunk) = response.chunk().await.map_err(map_send_error)? {
            if payload.len().saturating_add(chunk.len()) > self.max_storage_payload_bytes {
                return Err(RuntimeBrokerClientErrorCode::ResponseTooLarge.into());
            }
            payload.extend_from_slice(&chunk);
        }
        if payload.len() as u64 != payload_bytes
            || hex::encode(Sha256::digest(&payload)) != sha256_hex
        {
            return Err(RuntimeBrokerClientErrorCode::InvalidResponse.into());
        }
        Ok(RuntimeStorageOperationResponse {
            response: RuntimeOperationResponse {
                api_version: BrokerApiVersion::V1,
                request_id,
                result: RuntimeOperationResult::StoragePayload {
                    payload_bytes,
                    sha256_hex,
                },
            },
            payload: Some(payload),
        })
    }
}

fn validate_input(
    accepts_payload: bool,
    declared: Option<u64>,
    payload: Option<&[u8]>,
    max_payload_bytes: usize,
) -> Result<(), RuntimeBrokerClientError> {
    match (accepts_payload, declared, payload) {
        (true, Some(declared), Some(payload))
            if declared == payload.len() as u64 && payload.len() <= max_payload_bytes =>
        {
            Ok(())
        }
        (false, None, None) => Ok(()),
        _ => Err(RuntimeBrokerClientErrorCode::InvalidRequest.into()),
    }
}

fn required_header<'a>(
    headers: &'a reqwest::header::HeaderMap,
    name: &'static str,
) -> Result<&'a str, RuntimeBrokerClientError> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| RuntimeBrokerClientErrorCode::InvalidResponse.into())
}

fn content_type(headers: &reqwest::header::HeaderMap) -> Option<&str> {
    headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
}

fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn map_send_error(error: reqwest::Error) -> RuntimeBrokerClientError {
    if error.is_timeout() {
        RuntimeBrokerClientErrorCode::TimedOut.into()
    } else {
        RuntimeBrokerClientErrorCode::Unreachable.into()
    }
}

#[cfg(test)]
mod tests;
