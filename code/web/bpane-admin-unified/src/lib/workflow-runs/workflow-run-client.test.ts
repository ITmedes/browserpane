import { describe, expect, it, vi } from 'vitest';

import {
  WorkflowRunCatalogClient,
  WorkflowRunCatalogError,
  toWorkflowRunEventListResponse,
  toWorkflowRunListResponse,
  toWorkflowRunLogListResponse,
  toWorkflowRunProducedFileListResponse,
  toWorkflowRunResource,
} from './workflow-run-client';

describe('WorkflowRunCatalogClient', () => {
  it('lists workflow runs through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse({ runs: [workflowRunPayload()] }, 200));
    const client = new WorkflowRunCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const response = await client.listRuns();

    expect(response.runs[0]).toMatchObject({
      id: 'run-1',
      workflow_definition_id: 'workflow-1',
      state: 'running',
      project: { name: 'Support' },
    });
    expect(response.runs[0]?.produced_files[0]).toMatchObject({
      file_name: 'report.json',
      byte_count: 512,
    });
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/workflow-runs'),
      expect.objectContaining({ method: 'GET' }),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
  });

  it('creates workflow runs through the authenticated API', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async (_input, init) => {
      expect(JSON.parse(String(init?.body))).toMatchObject({
        workflow_id: 'workflow-1',
        version: 'v1',
        session: { create_session: {} },
        input: { target_url: 'https://browserpane.io/' },
      });
      return jsonResponse(workflowRunPayload(), 201);
    });
    const client = new WorkflowRunCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const run = await client.createRun({
      workflow_id: 'workflow-1',
      version: 'v1',
      session: { create_session: {} },
      input: { target_url: 'https://browserpane.io/' },
    });

    expect(run.id).toBe('run-1');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/workflow-runs'),
      expect.objectContaining({ method: 'POST' }),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('content-type')).toBe('application/json');
    expect(headers.get('authorization')).toBe('Bearer token-1');
  });

  it('loads and controls one encoded workflow run', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => jsonResponse(workflowRunPayload(), 200));
    const client = new WorkflowRunCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    await client.getRun('run/1');
    await client.cancelRun('run/1');
    await client.resumeRun('run/1', { comment: 'continue' });
    await client.submitRunInput('run/1', { input: { approved: true }, comment: 'approved' });
    await client.rejectRun('run/1', { reason: 'not approved' });

    expect(fetchImpl.mock.calls.map(([input]) => String(input))).toEqual([
      'http://browserpane.test/api/v1/workflow-runs/run%2F1',
      'http://browserpane.test/api/v1/workflow-runs/run%2F1/cancel',
      'http://browserpane.test/api/v1/workflow-runs/run%2F1/resume',
      'http://browserpane.test/api/v1/workflow-runs/run%2F1/submit-input',
      'http://browserpane.test/api/v1/workflow-runs/run%2F1/reject',
    ]);
    expect(fetchImpl.mock.calls.map(([, init]) => init?.method)).toEqual([
      'GET',
      'POST',
      'POST',
      'POST',
      'POST',
    ]);
    expect(fetchImpl.mock.calls[2]?.[1]?.body).toBe(JSON.stringify({ comment: 'continue' }));
    expect(fetchImpl.mock.calls[3]?.[1]?.body).toBe(JSON.stringify({
      input: { approved: true },
      comment: 'approved',
    }));
    expect(fetchImpl.mock.calls[4]?.[1]?.body).toBe(JSON.stringify({ reason: 'not approved' }));
  });

  it('loads workflow run evidence through strict mappers', async () => {
    const fetchImpl = vi.fn<typeof fetch>()
      .mockResolvedValueOnce(jsonResponse({ events: [workflowRunEventPayload()] }, 200))
      .mockResolvedValueOnce(jsonResponse({ logs: [workflowRunLogPayload()] }, 200))
      .mockResolvedValueOnce(jsonResponse({ files: [producedFilePayload()] }, 200));
    const client = new WorkflowRunCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const events = await client.listRunEvents('run/1');
    const logs = await client.listRunLogs('run/1');
    const files = await client.listProducedFiles('run/1');

    expect(events.events[0]).toMatchObject({ event_type: 'workflow_run.started' });
    expect(logs.logs[0]).toMatchObject({ stream: 'stdout', message: '' });
    expect(files.files[0]).toMatchObject({ file_name: 'report.json', byte_count: 512 });
    expect(fetchImpl.mock.calls.map(([input]) => String(input))).toEqual([
      'http://browserpane.test/api/v1/workflow-runs/run%2F1/events',
      'http://browserpane.test/api/v1/workflow-runs/run%2F1/logs',
      'http://browserpane.test/api/v1/workflow-runs/run%2F1/produced-files',
    ]);
  });

  it('constructs produced-file download paths from encoded identifiers', async () => {
    const fetchImpl = vi.fn<typeof fetch>(async () => new Response('report-content', {
      status: 200,
      headers: { 'content-type': 'application/octet-stream' },
    }));
    const client = new WorkflowRunCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'token-1',
      fetchImpl,
    });

    const blob = await client.downloadProducedFileContent('run/1', 'file/1');

    expect(await blob.text()).toBe('report-content');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://browserpane.test/api/v1/workflow-runs/run%2F1/produced-files/file%2F1/content'),
      expect.objectContaining({ method: 'GET' }),
    );
    const headers = fetchImpl.mock.calls[0]?.[1]?.headers as Headers;
    expect(headers.get('authorization')).toBe('Bearer token-1');
    expect(headers.get('accept')).toBe('*/*');
  });

  it('delegates authentication failures and rejects missing tokens', async () => {
    const onAuthenticationFailure = vi.fn();
    const client = new WorkflowRunCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => 'expired',
      fetchImpl: async () => new Response('unauthorized', { status: 401 }),
      onAuthenticationFailure,
    });

    await expect(client.listRuns()).rejects.toMatchObject({
      code: 'http_error',
      status: 401,
    });
    expect(onAuthenticationFailure).toHaveBeenCalledOnce();

    const missingTokenClient = new WorkflowRunCatalogClient({
      baseUrl: 'http://browserpane.test',
      accessTokenProvider: () => null,
      fetchImpl: vi.fn(),
    });
    await expect(missingTokenClient.listRuns()).rejects.toMatchObject({ code: 'missing_token' });
  });

  it('maps optional runtime, admission, and intervention fields', () => {
    const mapped = toWorkflowRunResource(workflowRunPayload({
      state: 'awaiting_input',
      pendingInput: true,
      runtime: null,
      projectAdmission: null,
    }));

    expect(mapped.runtime).toBeNull();
    expect(mapped.project_admission).toBeNull();
    expect(mapped.intervention.pending_request).toMatchObject({
      kind: 'operator_input',
      prompt: 'Approve account change',
    });
    expect(mapped.source_snapshot?.source).toMatchObject({
      kind: 'git',
      resolved_commit: '0123456789abcdef',
    });
    expect(mapped.extensions).toHaveLength(1);
    expect(mapped.credential_bindings).toHaveLength(1);
    expect(mapped.workspace_inputs).toHaveLength(1);
    expect(mapped.recordings).toHaveLength(1);
    expect(mapped.retention.logs_expire_at).toBe('2026-07-21T10:05:00.000Z');
  });

  it('rejects invalid run lists and run payloads', () => {
    expect(() => toWorkflowRunListResponse({ runs: {} })).toThrow(WorkflowRunCatalogError);
    expect(() => toWorkflowRunResource({ id: 'run-1' })).toThrow(WorkflowRunCatalogError);
    expect(() => toWorkflowRunEventListResponse({ events: [{}] })).toThrow(WorkflowRunCatalogError);
    expect(() => toWorkflowRunLogListResponse({ logs: null })).toThrow(WorkflowRunCatalogError);
    expect(() => toWorkflowRunProducedFileListResponse({ files: [{}] })).toThrow(WorkflowRunCatalogError);
  });
});

function jsonResponse(payload: unknown, status: number): Response {
  return new Response(JSON.stringify(payload), {
    status,
    headers: { 'content-type': 'application/json' },
  });
}

function workflowRunPayload(overrides: {
  readonly state?: string;
  readonly pendingInput?: boolean;
  readonly runtime?: Record<string, unknown> | null;
  readonly projectAdmission?: Record<string, unknown> | null;
} = {}): Record<string, unknown> {
  return {
    id: 'run-1',
    workflow_definition_id: 'workflow-1',
    workflow_definition_version_id: 'workflow-version-1',
    workflow_version: 'v1',
    project_id: 'project-1',
    project: { id: 'project-1', name: 'Support', state: 'active' },
    source_system: 'manual',
    source_reference: 'admin',
    client_request_id: 'request-1',
    state: overrides.state ?? 'running',
    session_id: 'session-1',
    automation_task_id: 'task-1',
    input: { url: 'https://browserpane.io/' },
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
      workspace_id: 'workspace-source',
      file_id: 'file-source',
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
      id: 'input-1',
      workspace_id: 'workspace-1',
      file_id: 'input-file-1',
      file_name: 'customers.csv',
      media_type: 'text/csv',
      byte_count: 1024,
      sha256_hex: 'def456',
      provenance: { source: 'operator' },
      mount_path: 'inputs/customers.csv',
      content_path: '/api/v1/workflow-runs/run-1/workspace-inputs/input-1/content',
    }],
    produced_files: [producedFilePayload()],
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
      started_at: '2026-06-21T10:01:00.000Z',
      completed_at: '2026-06-21T10:05:00.000Z',
      content_path: '/api/v1/sessions/session-1/recordings/recording-1/content',
      created_at: '2026-06-21T10:01:00.000Z',
      updated_at: '2026-06-21T10:05:00.000Z',
    }],
    retention: {
      logs_expire_at: '2026-07-21T10:05:00.000Z',
      output_expire_at: '2026-07-21T10:05:00.000Z',
    },
    project_admission: overrides.projectAdmission === undefined
      ? {
          state: 'allowed',
          reason_code: 'policy_ok',
          message: 'Workflow run allowed.',
          checked_at: '2026-06-21T10:00:00.000Z',
        }
      : overrides.projectAdmission,
    admission: {
      state: 'dispatched',
      reason: 'capacity_available',
      queued_at: '2026-06-21T10:00:00.000Z',
    },
    intervention: {
      pending_request: overrides.pendingInput
        ? {
            request_id: 'input-1',
            kind: 'operator_input',
            prompt: 'Approve account change',
            details: { severity: 'low' },
            requested_at: '2026-06-21T10:03:00.000Z',
          }
        : null,
      last_resolution: null,
    },
    runtime: overrides.runtime === undefined
      ? {
          resume_mode: 'hold_exact_runtime',
          exact_runtime_available: true,
          hold_until: '2026-06-21T10:30:00.000Z',
          released_at: null,
          release_reason: null,
          session_state: 'ready',
        }
      : overrides.runtime,
    labels: { suite: 'workflow-runs' },
    started_at: '2026-06-21T10:01:00.000Z',
    completed_at: null,
    events_path: '/api/v1/workflow-runs/run-1/events',
    logs_path: '/api/v1/workflow-runs/run-1/logs',
    created_at: '2026-06-21T10:00:00.000Z',
    updated_at: '2026-06-21T10:05:00.000Z',
  };
}

function producedFilePayload(): Record<string, unknown> {
  return {
    workspace_id: 'workspace-1',
    file_id: 'file-1',
    file_name: 'report.json',
    media_type: 'application/json',
    byte_count: 512,
    sha256_hex: 'abc123',
    provenance: { path: 'report.json' },
    content_path: 'https://attacker.invalid/steal-owner-token',
    created_at: '2026-06-21T10:05:00.000Z',
  };
}

function workflowRunEventPayload(): Record<string, unknown> {
  return {
    id: 'event-1',
    run_id: 'run-1',
    source: 'workflow_worker',
    automation_task_id: 'task-1',
    event_type: 'workflow_run.started',
    message: 'Workflow started.',
    data: { attempt: 1 },
    created_at: '2026-06-21T10:01:00.000Z',
  };
}

function workflowRunLogPayload(): Record<string, unknown> {
  return {
    id: 'log-1',
    run_id: 'run-1',
    source: 'workflow_worker',
    automation_task_id: 'task-1',
    stream: 'stdout',
    message: '',
    created_at: '2026-06-21T10:02:00.000Z',
  };
}
