use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{PolicyErrorCode, RuntimeOperationKind, RuntimeOperationRequest};

/// Sanitized outcome recorded for a broker operation.
#[derive(Debug, Clone, Copy, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditOutcome {
    /// Policy and authentication accepted the operation.
    Accepted,
    /// Policy or authentication denied the operation.
    Denied,
    /// An approved operation failed at an adapter boundary.
    Failed,
    /// A persisted operation was reconciled after restart.
    Reconciled,
}

/// Safe resource correlation included in broker audit events.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AuditResource {
    /// Product operation family.
    pub operation_kind: RuntimeOperationKind,
    /// Primary BrowserPane resource identifier.
    pub resource_id: Uuid,
}

/// Sanitized broker audit resource.
#[derive(Debug, Clone, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeBrokerAuditEvent {
    /// Request correlation identifier.
    pub request_id: Uuid,
    /// Idempotency-key fingerprint supplied by the caller of the builder.
    pub idempotency_key_fingerprint: String,
    /// Safe resource correlation.
    pub resource: AuditResource,
    /// Sanitized result category.
    pub outcome: AuditOutcome,
    /// Stable policy code when the outcome is a denial.
    pub policy_error_code: Option<PolicyErrorCode>,
}

/// Builder that copies only explicitly safe request metadata into an audit event.
pub struct RuntimeBrokerAuditEventBuilder<'a> {
    request: &'a RuntimeOperationRequest,
    idempotency_key_fingerprint: String,
}

impl<'a> RuntimeBrokerAuditEventBuilder<'a> {
    /// Starts an audit event from a typed operation request.
    pub fn new(
        request: &'a RuntimeOperationRequest,
        idempotency_key_fingerprint: impl Into<String>,
    ) -> Self {
        Self {
            request,
            idempotency_key_fingerprint: idempotency_key_fingerprint.into(),
        }
    }

    /// Records an accepted operation.
    pub fn accepted(self) -> RuntimeBrokerAuditEvent {
        self.build(AuditOutcome::Accepted, None)
    }

    /// Records a policy-denied operation using only its stable code.
    pub fn denied(self, code: PolicyErrorCode) -> RuntimeBrokerAuditEvent {
        self.build(AuditOutcome::Denied, Some(code))
    }

    /// Records an adapter failure without copying raw adapter output.
    pub fn failed(self) -> RuntimeBrokerAuditEvent {
        self.build(AuditOutcome::Failed, None)
    }

    /// Records a reconciled operation.
    pub fn reconciled(self) -> RuntimeBrokerAuditEvent {
        self.build(AuditOutcome::Reconciled, None)
    }

    fn build(
        self,
        outcome: AuditOutcome,
        policy_error_code: Option<PolicyErrorCode>,
    ) -> RuntimeBrokerAuditEvent {
        RuntimeBrokerAuditEvent {
            request_id: self.request.request_id,
            idempotency_key_fingerprint: self.idempotency_key_fingerprint,
            resource: AuditResource {
                operation_kind: self.request.operation.kind(),
                resource_id: self.request.operation.resource_id(),
            },
            outcome,
            policy_error_code,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        BrokerApiVersion, IdempotencyKey, RuntimeOperation, SecretValue, WorkflowWorkerCredentials,
        WorkflowWorkerLaunchRequest,
    };

    #[test]
    fn audit_event_never_serializes_operation_secrets() {
        let secret = "workflow-secret-never-audit";
        let run_id = Uuid::now_v7();
        let request = RuntimeOperationRequest {
            api_version: BrokerApiVersion::V1,
            request_id: Uuid::now_v7(),
            idempotency_key: IdempotencyKey::new("workflow:launch:1").unwrap(),
            operation: RuntimeOperation::LaunchWorkflow(WorkflowWorkerLaunchRequest {
                workflow_run_id: run_id,
                session_id: Uuid::now_v7(),
                automation_task_id: Uuid::now_v7(),
                credentials: WorkflowWorkerCredentials {
                    session_automation_access_token: SecretValue::new(secret).unwrap(),
                    gateway_bearer_token: None,
                },
            }),
        };

        let event = RuntimeBrokerAuditEventBuilder::new(&request, "sha256:key").accepted();
        let json = serde_json::to_string(&event).unwrap();

        assert!(!json.contains(secret));
        assert!(!json.contains("credentials"));
        assert!(!json.contains(request.idempotency_key.as_str()));
        assert_eq!(event.resource.resource_id, run_id);
    }
}
