use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Map as JsonMap, Value};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};
use tokio_postgres::{NoTls, Row, Transaction};
use uuid::Uuid;

use crate::auth::AuthenticatedPrincipal;
use crate::automation_task::{
    AutomationTaskLogStream, AutomationTaskSessionSource, AutomationTaskState,
    AutomationTaskTransitionRequest, PersistAutomationTaskRequest, StoredAutomationTask,
    StoredAutomationTaskEvent, StoredAutomationTaskLog,
};
use crate::credentials::{
    CredentialBindingProvider, CredentialInjectionMode, CredentialTotpMetadata,
    PersistCredentialBindingRequest, StoredCredentialBinding, WorkflowRunCredentialBinding,
};
use crate::extension::{
    AppliedExtension, PersistExtensionDefinitionRequest, PersistExtensionVersionRequest,
    StoredExtensionDefinition, StoredExtensionVersion,
};
use crate::session_manager::{
    PersistedSessionRuntimeAssignment, SessionManagerProfile, SessionRuntimeAccess,
    SessionRuntimeAssignmentStatus,
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

#[cfg(test)]
mod tests;
