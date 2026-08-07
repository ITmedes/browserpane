import type {
  WorkflowRunEventResource,
  WorkflowRunLogResource,
  WorkflowRunProducedFileResource,
  WorkflowRunResource,
} from '$lib/workflow-runs/workflow-run-types';

export function workflowRunFixture(overrides: Partial<WorkflowRunResource> = {}): WorkflowRunResource {
  return {
    id: 'run-1',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'workflow-version-1',
    workflow_version: 'v1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    source_system: 'external_scheduler',
    source_reference: 'ticket-42',
    client_request_id: 'request-1',
    state: 'awaiting_input',
    session_id: 'session-1',
    automation_task_id: 'task-1',
    input: { target: 'https://example.com' },
    output: null,
    error: null,
    artifact_refs: ['artifact-1'],
    source_snapshot: {
      source: {
        kind: 'git',
        repository_url: 'https://github.com/ITmedes/browserpane.git',
        ref: 'main',
        resolved_commit: '0123456789abcdef',
        root_path: 'dev',
      },
      entrypoint: 'workflows/report/run.ts',
      workspace_id: 'source-workspace-1',
      file_id: 'source-file-1',
      file_name: 'workflow-source.zip',
      media_type: 'application/zip',
      content_path: '/api/v1/workflow-runs/run-1/source-snapshot/content',
    },
    extensions: [{
      extension_id: 'extension-1',
      extension_version_id: 'extension-version-1',
      name: 'Password manager',
      version: '1.0.0',
    }],
    credential_bindings: [{
      id: 'binding-1',
      project_id: 'project-1',
      name: 'Support credentials',
      provider: 'vault_kv2',
      namespace: 'support',
      allowed_origins: ['https://example.com'],
      injection_mode: 'form_fill',
      totp: null,
      resolve_path: '/api/v1/workflow-runs/run-1/credential-bindings/binding-1/resolved',
    }],
    workspace_inputs: [{
      id: 'workspace-input-1',
      workspace_id: 'workspace-1',
      file_id: 'input-file-1',
      file_name: 'customers.csv',
      media_type: 'text/csv',
      byte_count: 1024,
      sha256_hex: 'input-sha',
      provenance: { source: 'operator' },
      mount_path: 'inputs/customers.csv',
      content_path: '/api/v1/workflow-runs/run-1/workspace-inputs/workspace-input-1/content',
    }],
    produced_files: [workflowRunProducedFileFixture()],
    recordings: [{
      id: 'recording-1',
      session_id: 'session-1',
      state: 'ready',
      format: 'webm',
      mime_type: 'video/webm',
      bytes: 2048,
      duration_ms: 5000,
      error: null,
      termination_reason: 'session_stopped',
      previous_recording_id: null,
      started_at: '2026-08-07T10:01:00.000Z',
      completed_at: '2026-08-07T10:05:00.000Z',
      content_path: '/api/v1/sessions/session-1/recordings/recording-1/content',
      created_at: '2026-08-07T10:01:00.000Z',
      updated_at: '2026-08-07T10:05:00.000Z',
    }],
    retention: {
      logs_expire_at: '2026-09-07T10:05:00.000Z',
      output_expire_at: '2026-09-07T10:05:00.000Z',
    },
    project_admission: {
      state: 'allowed',
      reason_code: 'policy_ok',
      message: 'Workflow run allowed.',
      active_workflow_runs: 1,
      max_active_workflow_runs: 4,
      checked_at: '2026-08-07T10:00:00.000Z',
    },
    admission: null,
    intervention: {
      pending_request: {
        request_id: 'intervention-1',
        kind: 'approval',
        prompt: 'Approve the next step',
        details: { risk: 'low' },
        requested_at: '2026-08-07T10:05:00.000Z',
      },
      last_resolution: null,
    },
    runtime: {
      resume_mode: 'live_runtime',
      exact_runtime_available: true,
      hold_until: '2026-08-07T10:35:00.000Z',
      released_at: null,
      release_reason: null,
      session_state: 'ready',
    },
    labels: { suite: 'workflow-run-detail' },
    started_at: '2026-08-07T10:01:00.000Z',
    completed_at: null,
    events_path: '/api/v1/workflow-runs/run-1/events',
    logs_path: '/api/v1/workflow-runs/run-1/logs',
    created_at: '2026-08-07T10:00:00.000Z',
    updated_at: '2026-08-07T10:05:00.000Z',
    ...overrides,
  };
}

export function workflowRunEventFixture(): WorkflowRunEventResource {
  return {
    id: 'event-1',
    run_id: 'run-1',
    source: 'workflow_worker',
    automation_task_id: 'task-1',
    event_type: 'workflow_run.awaiting_input',
    message: 'Operator approval required.',
    data: { attempt: 1 },
    created_at: '2026-08-07T10:05:00.000Z',
  };
}

export function workflowRunLogFixture(): WorkflowRunLogResource {
  return {
    id: 'log-1',
    run_id: 'run-1',
    source: 'workflow_worker',
    automation_task_id: 'task-1',
    stream: 'stdout',
    message: 'Waiting for operator approval.',
    created_at: '2026-08-07T10:05:00.000Z',
  };
}

export function workflowRunProducedFileFixture(): WorkflowRunProducedFileResource {
  return {
    workspace_id: 'workspace-1',
    file_id: 'file-1',
    file_name: 'report.json',
    media_type: 'application/json',
    byte_count: 512,
    sha256_hex: 'output-sha',
    provenance: { source: 'workflow' },
    content_path: '/api/v1/workflow-runs/run-1/produced-files/file-1/content',
    created_at: '2026-08-07T10:05:00.000Z',
  };
}
