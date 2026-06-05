use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde_json::{Map as JsonMap, Value};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio_postgres::{NoTls, Row, Transaction};
use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;
use crate::automation_tasks::{
    AutomationTaskLogStream, AutomationTaskSessionSource, AutomationTaskState,
    AutomationTaskTransitionRequest, PersistAutomationTaskRequest, StoredAutomationTask,
    StoredAutomationTaskEvent, StoredAutomationTaskLog,
};
use crate::credentials::{
    CredentialBindingProvider, CredentialInjectionMode, CredentialTotpMetadata,
    PersistCredentialBindingRequest, StoredCredentialBinding, WorkflowRunCredentialBinding,
};
use crate::extensions::{
    AppliedExtension, PersistExtensionDefinitionRequest, PersistExtensionVersionRequest,
    StoredExtensionDefinition, StoredExtensionVersion,
};
pub use crate::session_files::{
    PersistSessionFileBindingRequest, PersistSessionFileRequest, SessionFileBindingMode,
    SessionFileBindingState, SessionFileRetentionCandidate, SessionFileSource, StoredSessionFile,
    StoredSessionFileBinding,
};
use crate::session_manager::{
    PersistedSessionRuntimeAssignment, SessionManagerProfile, SessionRuntimeAssignmentStatus,
};
use crate::workflow::WorkflowSource;
use crate::workflow::{
    automation_task_default_message_for_run_state, automation_task_event_type_for_run_state,
    workflow_run_default_message, workflow_run_event_type, CreateWorkflowRunResult,
    PersistWorkflowDefinitionRequest, PersistWorkflowDefinitionVersionRequest,
    PersistWorkflowRunEventRequest, PersistWorkflowRunLogRequest,
    PersistWorkflowRunProducedFileRequest, PersistWorkflowRunRequest, StoredWorkflowDefinition,
    StoredWorkflowDefinitionVersion, StoredWorkflowRun, StoredWorkflowRunEvent,
    StoredWorkflowRunLog, WorkflowRunProducedFile, WorkflowRunSourceSnapshot, WorkflowRunState,
    WorkflowRunTransitionRequest, WorkflowRunWorkspaceInput,
};
use crate::workflow_event_delivery::{
    build_workflow_event_delivery_payload, validate_workflow_event_subscription_request,
    workflow_event_type_matches, PersistWorkflowEventSubscriptionRequest,
    RecordWorkflowEventDeliveryAttemptRequest, StoredWorkflowEventDelivery,
    StoredWorkflowEventDeliveryAttempt, StoredWorkflowEventSubscription,
    WorkflowEventDeliveryState,
};
use crate::workspaces::{
    PersistFileWorkspaceFileRequest, PersistFileWorkspaceRequest, StoredFileWorkspace,
    StoredFileWorkspaceFile,
};

mod automation_task_policy;
mod extensions_store;
mod file_workspaces_store;
mod in_memory;
mod migrations;
mod postgres;
mod rows;
mod store;
mod types;
mod validation;
mod workflow_event_delivery_planning;
mod workflow_run_policy;

use automation_task_policy::*;
use in_memory::*;
use migrations::*;
use postgres::*;
use rows::*;
use validation::*;
use workflow_event_delivery_planning::*;
use workflow_run_policy::*;

pub use store::*;
pub use types::*;

fn session_visible_to_principal(
    session: &StoredSession,
    principal: &AuthenticatedPrincipal,
) -> bool {
    if session.owner.subject == principal.subject && session.owner.issuer == principal.issuer {
        return true;
    }

    let Some(delegate) = &session.automation_delegate else {
        return false;
    };

    principal.client_id.as_deref() == Some(delegate.client_id.as_str())
        && principal.issuer == delegate.issuer
}

fn task_visible_to_principal(session: &StoredSession, principal: &AuthenticatedPrincipal) -> bool {
    session.owner.subject == principal.subject && session.owner.issuer == principal.issuer
}

fn project_admission_conflict(decision: ProjectAdmissionDecision) -> SessionStoreError {
    SessionStoreError::Conflict(format!(
        "project admission rejected: {}: {}",
        decision.reason_code.as_str(),
        decision.message
    ))
}

fn validate_project_retained_storage_quota(
    project_id: Uuid,
    retained_storage_bytes: u64,
    incoming_bytes: u64,
    max_retained_storage_bytes: u64,
) -> Result<(), SessionStoreError> {
    let projected_storage_bytes = retained_storage_bytes
        .checked_add(incoming_bytes)
        .unwrap_or(u64::MAX);
    if projected_storage_bytes <= max_retained_storage_bytes {
        return Ok(());
    }

    Err(SessionStoreError::Conflict(format!(
        "retained_storage_quota_exceeded: project {project_id} retained storage quota would be exceeded ({projected_storage_bytes}/{max_retained_storage_bytes} bytes)"
    )))
}

fn validate_project_session_policy(
    project: &StoredProject,
    request: &CreateSessionRequest,
    active_project_sessions: u32,
    checked_at: DateTime<Utc>,
) -> Result<(), SessionStoreError> {
    if !project.policy.allowed_session_template_ids.is_empty() {
        let template_id = request.template_id.as_deref();
        let allowed = template_id.is_some_and(|template_id| {
            project
                .policy
                .allowed_session_template_ids
                .iter()
                .any(|allowed_id| allowed_id == template_id)
        });
        if !allowed {
            let message = match template_id {
                Some(template_id) => format!(
                    "project {} does not allow session template {}",
                    project.id, template_id
                ),
                None => format!(
                    "project {} requires one of the allowed session templates",
                    project.id
                ),
            };
            return Err(project_admission_conflict(
                ProjectAdmissionDecision::rejected(
                    project.id,
                    ProjectAdmissionReasonCode::SessionTemplateNotAllowed,
                    message,
                    active_project_sessions,
                    project.quotas.max_active_sessions,
                    checked_at,
                ),
            ));
        }
    }

    if !project.policy.allowed_egress_profile_ids.is_empty() {
        let egress_profile_id = request
            .network_identity
            .as_ref()
            .and_then(|identity| identity.egress_profile_id);
        let allowed = egress_profile_id.is_some_and(|profile_id| {
            project
                .policy
                .allowed_egress_profile_ids
                .iter()
                .any(|allowed_id| *allowed_id == profile_id)
        });
        if !allowed {
            let message = match egress_profile_id {
                Some(profile_id) => format!(
                    "project {} does not allow egress profile {}",
                    project.id, profile_id
                ),
                None => format!(
                    "project {} requires one of the allowed egress profiles",
                    project.id
                ),
            };
            return Err(project_admission_conflict(
                ProjectAdmissionDecision::rejected(
                    project.id,
                    ProjectAdmissionReasonCode::EgressProfileNotAllowed,
                    message,
                    active_project_sessions,
                    project.quotas.max_active_sessions,
                    checked_at,
                ),
            ));
        }
    }

    Ok(())
}

fn validate_project_session_creation_budget(
    project: &StoredProject,
    session_creations: u32,
    checked_at: DateTime<Utc>,
) -> Result<(), SessionStoreError> {
    if project.policy.usage_budget_enforcement
        != ProjectUsageBudgetEnforcement::BlockSessionCreation
    {
        return Ok(());
    }
    let Some(max_session_creations) = project.quotas.max_session_creations else {
        return Ok(());
    };
    if session_creations < max_session_creations {
        return Ok(());
    }

    Err(project_admission_conflict(
        ProjectAdmissionDecision::session_creation_budget_rejected(
            project.id,
            session_creations,
            max_session_creations,
            checked_at,
        ),
    ))
}

fn project_session_creation_window_start(
    project: &StoredProject,
    checked_at: DateTime<Utc>,
) -> Option<DateTime<Utc>> {
    let window_sec = project.quotas.session_creation_window_sec?;
    Some(checked_at - ChronoDuration::seconds(i64::from(window_sec)))
}

fn validate_project_session_creation_rate(
    project: &StoredProject,
    session_creations_in_window: u32,
    checked_at: DateTime<Utc>,
) -> Result<(), SessionStoreError> {
    if project.policy.usage_budget_enforcement
        != ProjectUsageBudgetEnforcement::BlockSessionCreation
    {
        return Ok(());
    }
    let (Some(max_session_creations_per_window), Some(session_creation_window_sec)) = (
        project.quotas.max_session_creations_per_window,
        project.quotas.session_creation_window_sec,
    ) else {
        return Ok(());
    };
    if session_creations_in_window < max_session_creations_per_window {
        return Ok(());
    }

    Err(project_admission_conflict(
        ProjectAdmissionDecision::session_creation_rate_rejected(
            project.id,
            session_creations_in_window,
            max_session_creations_per_window,
            session_creation_window_sec,
            checked_at,
        ),
    ))
}

fn validate_project_runtime_usage_budget(
    project: &StoredProject,
    runtime_usage_ms: u64,
    checked_at: DateTime<Utc>,
) -> Result<(), SessionStoreError> {
    if project.policy.usage_budget_enforcement
        != ProjectUsageBudgetEnforcement::BlockSessionCreation
    {
        return Ok(());
    }
    let Some(max_runtime_usage_ms) = project.quotas.max_runtime_usage_ms else {
        return Ok(());
    };
    if runtime_usage_ms < max_runtime_usage_ms {
        return Ok(());
    }

    Err(project_admission_conflict(
        ProjectAdmissionDecision::runtime_usage_budget_rejected(
            project.id,
            runtime_usage_ms,
            max_runtime_usage_ms,
            checked_at,
        ),
    ))
}

#[cfg(test)]
mod tests;
