use super::support::principal;
use super::*;

mod idempotency;
mod state_sync;

struct WorkflowFixture {
    session: StoredSession,
    task: StoredAutomationTask,
    workflow: StoredWorkflowDefinition,
    version: StoredWorkflowDefinitionVersion,
}

async fn create_workflow_fixture(
    store: &SessionStore,
    owner: &AuthenticatedPrincipal,
    workflow_name: &str,
    task_name: &str,
) -> WorkflowFixture {
    let session = store
        .create_session(
            owner,
            CreateSessionRequest {
                template_id: None,
                owner_mode: None,
                viewport: None,
                idle_timeout_sec: None,
                labels: HashMap::new(),
                integration_context: None,
                extension_ids: Vec::new(),
                extensions: Vec::new(),
                recording: SessionRecordingPolicy::default(),
            },
            SessionOwnerMode::Collaborative,
        )
        .await
        .unwrap();
    let task = store
        .create_automation_task(
            owner,
            PersistAutomationTaskRequest {
                display_name: Some(task_name.to_string()),
                executor: "playwright".to_string(),
                session_id: session.id,
                session_source: AutomationTaskSessionSource::CreatedSession,
                input: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let workflow = store
        .create_workflow_definition(
            owner,
            PersistWorkflowDefinitionRequest {
                name: workflow_name.to_string(),
                description: None,
                labels: HashMap::new(),
            },
        )
        .await
        .unwrap();
    let version = store
        .create_workflow_definition_version(
            owner,
            PersistWorkflowDefinitionVersionRequest {
                workflow_definition_id: workflow.id,
                version: "v1".to_string(),
                executor: "playwright".to_string(),
                entrypoint: "workflows/run.mjs".to_string(),
                source: None,
                input_schema: None,
                output_schema: None,
                default_session: None,
                allowed_credential_binding_ids: Vec::new(),
                allowed_extension_ids: Vec::new(),
                allowed_file_workspace_ids: Vec::new(),
            },
        )
        .await
        .unwrap();

    WorkflowFixture {
        session,
        task,
        workflow,
        version,
    }
}
