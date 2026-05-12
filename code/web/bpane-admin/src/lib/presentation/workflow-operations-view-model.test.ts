import { describe, expect, it } from 'vitest';
import type { SessionResource } from '../api/control-types';
import type {
  WorkflowDefinitionResource,
  WorkflowDefinitionVersionResource,
  WorkflowRunResource,
} from '../api/workflow-types';
import { WorkflowOperationsViewModelBuilder } from './workflow-operations-view-model';

describe('WorkflowOperationsViewModelBuilder', () => {
  it('keeps run controls disabled until a session and definition are selected', () => {
    const viewModel = WorkflowOperationsViewModelBuilder.build({
      selectedSession: null,
      definitions: [],
      selectedWorkflowId: '',
      selectedVersion: '',
      selectedVersionResource: null,
      currentRun: null,
      logs: [],
      events: [],
      files: [],
      loading: false,
      actionInFlight: false,
      error: null,
      inputValid: true,
      connected: false,
      interventionInputValid: true,
    });

    expect(viewModel.status).toBe('ready');
    expect(viewModel.canRun).toBe(false);
    expect(viewModel.canCreateBaseline).toBe(true);
    expect(viewModel.canConnectBaseline).toBe(false);
    expect(viewModel.note).toContain('Select or create');
    expect(viewModel.invokeBlockedReason).toContain('selected session is the workflow baseline');
  });

  it('enables invocation for a selected session and workflow version', () => {
    const viewModel = WorkflowOperationsViewModelBuilder.build({
      selectedSession: SESSION,
      definitions: [WORKFLOW],
      selectedWorkflowId: WORKFLOW.id,
      selectedVersion: 'v1',
      selectedVersionResource: VERSION,
      currentRun: null,
      logs: [],
      events: [],
      files: [],
      loading: false,
      actionInFlight: false,
      error: null,
      inputValid: true,
      connected: true,
      interventionInputValid: true,
    });

    expect(viewModel.definitionOptions[0]?.label).toBe('operator-check (v1)');
    expect(viewModel.executorLabel).toBe('manual');
    expect(viewModel.selectedSessionLabel).toBe(SESSION.id);
    expect(viewModel.runSessionLabel).toBe('--');
    expect(viewModel.canRun).toBe(true);
    expect(viewModel.canCreateBaseline).toBe(false);
    expect(viewModel.canConnectBaseline).toBe(false);
    expect(viewModel.invokeBlockedReason).toBeNull();
  });

  it('requires the selected baseline session to be connected in the admin view', () => {
    const viewModel = WorkflowOperationsViewModelBuilder.build({
      selectedSession: SESSION,
      definitions: [WORKFLOW],
      selectedWorkflowId: WORKFLOW.id,
      selectedVersion: 'v1',
      selectedVersionResource: VERSION,
      currentRun: null,
      logs: [],
      events: [],
      files: [],
      loading: false,
      actionInFlight: false,
      error: null,
      inputValid: true,
      connected: false,
      interventionInputValid: true,
    });

    expect(viewModel.canRun).toBe(false);
    expect(viewModel.canConnectBaseline).toBe(true);
    expect(viewModel.invokeBlockedReason).toBe(
      'Connect the selected session before invoking a workflow from the admin view.',
    );
  });

  it('explains missing definitions and invalid run input before invocation', () => {
    const missingDefinition = WorkflowOperationsViewModelBuilder.build({
      selectedSession: SESSION,
      definitions: [],
      selectedWorkflowId: '',
      selectedVersion: '',
      selectedVersionResource: null,
      currentRun: null,
      logs: [],
      events: [],
      files: [],
      loading: false,
      actionInFlight: false,
      error: null,
      inputValid: true,
      connected: true,
      interventionInputValid: true,
    });
    expect(missingDefinition.canRun).toBe(false);
    expect(missingDefinition.invokeBlockedReason).toContain('workflow definition and version');

    const invalidInput = WorkflowOperationsViewModelBuilder.build({
      selectedSession: SESSION,
      definitions: [WORKFLOW],
      selectedWorkflowId: WORKFLOW.id,
      selectedVersion: 'v1',
      selectedVersionResource: VERSION,
      currentRun: null,
      logs: [],
      events: [],
      files: [],
      loading: false,
      actionInFlight: false,
      error: null,
      inputValid: false,
      connected: true,
      interventionInputValid: true,
    });
    expect(invalidInput.canRun).toBe(false);
    expect(invalidInput.invokeBlockedReason).toBe('Run input must be valid JSON.');
  });

  it('enables intervention actions only for awaiting-input runs', () => {
    const viewModel = WorkflowOperationsViewModelBuilder.build({
      selectedSession: SESSION,
      definitions: [WORKFLOW],
      selectedWorkflowId: WORKFLOW.id,
      selectedVersion: 'v1',
      selectedVersionResource: VERSION,
      currentRun: {
        ...RUN,
        state: 'awaiting_input',
        intervention: {
          pending_request: {
            request_id: 'request-1',
            kind: 'approval',
            prompt: 'Approve checkout?',
            requested_at: '2026-05-04T19:04:00Z',
          },
        },
      },
      logs: [{ id: 'log-1', run_id: RUN.id, source: 'run', stream: 'stdout', message: 'hello',
        created_at: RUN.created_at }],
      events: [],
      files: [],
      loading: false,
      actionInFlight: false,
      error: null,
      inputValid: true,
      connected: true,
      interventionInputValid: true,
    });

    expect(viewModel.status).toBe('awaiting_input');
    expect(viewModel.pendingPrompt).toBe('Approve checkout?');
    expect(viewModel.runSessionLabel).toBe(SESSION.id);
    expect(viewModel.runSessionNote).toBe('Run uses the selected baseline session.');
    expect(viewModel.canReleaseHold).toBe(true);
    expect(viewModel.canSubmitInput).toBe(true);
    expect(viewModel.logCount).toBe(1);
  });
});

const SESSION: SessionResource = {
  id: 'session-1',
  state: 'active',
  owner_mode: 'shared',
  connect: {
    gateway_url: 'https://localhost:4433',
    transport_path: '/session',
    auth_type: 'session_connect_ticket',
    compatibility_mode: 'session_runtime_pool',
  },
  runtime: {
    binding: 'docker_runtime_pool',
    compatibility_mode: 'session_runtime_pool',
  },
  status: {
    runtime_state: 'running',
    presence_state: 'connected',
    connection_counts: {
      interactive_clients: 1,
      owner_clients: 1,
      viewer_clients: 0,
      recorder_clients: 0,
      automation_clients: 0,
      total_clients: 1,
    },
    stop_eligibility: {
      allowed: false,
      blockers: [],
    },
  },
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

const WORKFLOW: WorkflowDefinitionResource = {
  id: 'workflow-1',
  name: 'operator-check',
  description: null,
  labels: {},
  latest_version: 'v1',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:00:00Z',
};

const VERSION: WorkflowDefinitionVersionResource = {
  id: 'version-1',
  workflow_definition_id: WORKFLOW.id,
  version: 'v1',
  executor: 'manual',
  entrypoint: 'workflows/operator/run.mjs',
  input_schema: null,
  output_schema: null,
  default_session: null,
  allowed_credential_binding_ids: [],
  allowed_extension_ids: [],
  allowed_file_workspace_ids: [],
  created_at: '2026-05-04T19:00:00Z',
};

const RUN: WorkflowRunResource = {
  id: 'run-1',
  workflow_definition_id: WORKFLOW.id,
  workflow_definition_version_id: VERSION.id,
  workflow_version: 'v1',
  state: 'queued',
  session_id: SESSION.id,
  automation_task_id: 'task-1',
  input: {},
  output: null,
  error: null,
  artifact_refs: [],
  produced_files: [],
  intervention: { pending_request: null },
  runtime: null,
  labels: {},
  events_path: '/api/v1/workflow-runs/run-1/events',
  logs_path: '/api/v1/workflow-runs/run-1/logs',
  created_at: '2026-05-04T19:03:00Z',
  updated_at: '2026-05-04T19:03:00Z',
};
