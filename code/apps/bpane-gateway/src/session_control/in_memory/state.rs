use super::*;

pub(in crate::session_control) struct InMemoryStoreState {
    pub(in crate::session_control) sessions: Mutex<Vec<StoredSession>>,
    pub(in crate::session_control) automation_tasks: Mutex<Vec<StoredAutomationTask>>,
    pub(in crate::session_control) automation_task_events: Mutex<Vec<StoredAutomationTaskEvent>>,
    pub(in crate::session_control) automation_task_logs: Mutex<Vec<StoredAutomationTaskLog>>,
    pub(in crate::session_control) workflow_definitions: Mutex<Vec<StoredWorkflowDefinition>>,
    pub(in crate::session_control) workflow_definition_versions:
        Mutex<Vec<StoredWorkflowDefinitionVersion>>,
    pub(in crate::session_control) workflow_runs: Mutex<Vec<StoredWorkflowRun>>,
    pub(in crate::session_control) workflow_run_events: Mutex<Vec<StoredWorkflowRunEvent>>,
    pub(in crate::session_control) workflow_run_logs: Mutex<Vec<StoredWorkflowRunLog>>,
    pub(in crate::session_control) workflow_event_subscriptions:
        Mutex<Vec<StoredWorkflowEventSubscription>>,
    pub(in crate::session_control) workflow_event_deliveries:
        Mutex<Vec<StoredWorkflowEventDelivery>>,
    pub(in crate::session_control) workflow_event_delivery_attempts:
        Mutex<Vec<StoredWorkflowEventDeliveryAttempt>>,
    pub(in crate::session_control) credential_bindings: Mutex<Vec<StoredCredentialBinding>>,
    pub(in crate::session_control) extension_definitions: Mutex<Vec<StoredExtensionDefinition>>,
    pub(in crate::session_control) extension_versions: Mutex<Vec<StoredExtensionVersion>>,
    pub(in crate::session_control) file_workspaces: Mutex<Vec<StoredFileWorkspace>>,
    pub(in crate::session_control) file_workspace_files: Mutex<Vec<StoredFileWorkspaceFile>>,
    pub(in crate::session_control) session_file_bindings: Mutex<Vec<StoredSessionFileBinding>>,
    pub(in crate::session_control) recordings: Mutex<Vec<StoredSessionRecording>>,
    pub(in crate::session_control) runtime_assignments:
        Mutex<HashMap<Uuid, PersistedSessionRuntimeAssignment>>,
    pub(in crate::session_control) recording_worker_assignments:
        Mutex<HashMap<Uuid, PersistedSessionRecordingWorkerAssignment>>,
    pub(in crate::session_control) workflow_run_worker_assignments:
        Mutex<HashMap<Uuid, PersistedWorkflowRunWorkerAssignment>>,
}

impl InMemoryStoreState {
    pub(super) fn new() -> Self {
        Self {
            sessions: Mutex::new(Vec::new()),
            automation_tasks: Mutex::new(Vec::new()),
            automation_task_events: Mutex::new(Vec::new()),
            automation_task_logs: Mutex::new(Vec::new()),
            workflow_definitions: Mutex::new(Vec::new()),
            workflow_definition_versions: Mutex::new(Vec::new()),
            workflow_runs: Mutex::new(Vec::new()),
            workflow_run_events: Mutex::new(Vec::new()),
            workflow_run_logs: Mutex::new(Vec::new()),
            workflow_event_subscriptions: Mutex::new(Vec::new()),
            workflow_event_deliveries: Mutex::new(Vec::new()),
            workflow_event_delivery_attempts: Mutex::new(Vec::new()),
            credential_bindings: Mutex::new(Vec::new()),
            extension_definitions: Mutex::new(Vec::new()),
            extension_versions: Mutex::new(Vec::new()),
            file_workspaces: Mutex::new(Vec::new()),
            file_workspace_files: Mutex::new(Vec::new()),
            session_file_bindings: Mutex::new(Vec::new()),
            recordings: Mutex::new(Vec::new()),
            runtime_assignments: Mutex::new(HashMap::new()),
            recording_worker_assignments: Mutex::new(HashMap::new()),
            workflow_run_worker_assignments: Mutex::new(HashMap::new()),
        }
    }
}
