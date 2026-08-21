use axum::http::{header::CONTENT_TYPE, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use crate::session_control::SessionStoreError;
use crate::workflow_endpoints::WorkflowSchemaViolation;

#[derive(Debug, Clone, Serialize)]
pub(super) struct ProblemDetails {
    #[serde(rename = "type")]
    type_url: String,
    title: String,
    status: u16,
    detail: String,
    code: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<WorkflowSchemaViolation>,
}

#[derive(Debug, Clone)]
pub(super) struct WorkflowEndpointApiError {
    status: StatusCode,
    problem: Box<ProblemDetails>,
}

impl WorkflowEndpointApiError {
    pub(super) fn new(
        status: StatusCode,
        code: &str,
        title: &str,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            status,
            problem: Box::new(ProblemDetails {
                type_url: format!("https://browserpane.dev/problems/{code}"),
                title: title.to_string(),
                status: status.as_u16(),
                detail: detail.into(),
                code: code.to_string(),
                instance: None,
                errors: Vec::new(),
            }),
        }
    }

    pub(super) fn with_validation_errors(mut self, errors: Vec<WorkflowSchemaViolation>) -> Self {
        self.problem.errors = errors;
        self
    }

    pub(super) fn from_store(error: SessionStoreError) -> Self {
        let (status, code, title) = match &error {
            SessionStoreError::ActiveSessionConflict { .. } | SessionStoreError::Conflict(_) => {
                (StatusCode::CONFLICT, "conflict", "Request conflict")
            }
            SessionStoreError::NotFound(_) => {
                (StatusCode::NOT_FOUND, "not_found", "Resource not found")
            }
            SessionStoreError::InvalidRequest(_) => (
                StatusCode::BAD_REQUEST,
                "invalid_request",
                "Invalid request",
            ),
            SessionStoreError::Backend(_) => (
                StatusCode::SERVICE_UNAVAILABLE,
                "service_unavailable",
                "Service unavailable",
            ),
        };
        let detail = match error {
            SessionStoreError::Backend(_) => {
                "the workflow endpoint store is temporarily unavailable".to_string()
            }
            other => other.to_string(),
        };
        Self::new(status, code, title, detail)
    }

    pub(super) fn from_legacy(error: (StatusCode, Json<super::super::ErrorResponse>)) -> Self {
        let (status, Json(error)) = error;
        let code = match status {
            StatusCode::BAD_REQUEST => "invalid_request",
            StatusCode::NOT_FOUND => "not_found",
            StatusCode::CONFLICT => "conflict",
            StatusCode::UNAUTHORIZED => "authentication_failed",
            StatusCode::FORBIDDEN => "authorization_denied",
            _ => "workflow_creation_failed",
        };
        Self::new(status, code, "Workflow invocation failed", error.error)
    }
}

impl IntoResponse for WorkflowEndpointApiError {
    fn into_response(self) -> Response {
        let mut response = (self.status, Json(self.problem)).into_response();
        response.headers_mut().insert(
            CONTENT_TYPE,
            HeaderValue::from_static("application/problem+json"),
        );
        response
    }
}
