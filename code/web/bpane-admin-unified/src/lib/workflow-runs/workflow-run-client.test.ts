import { describe, expect, it, vi } from 'vitest';

import {
  WorkflowRunCatalogClient,
  WorkflowRunCatalogError,
  toWorkflowRunListResponse,
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
  });

  it('rejects invalid run lists and run payloads', () => {
    expect(() => toWorkflowRunListResponse({ runs: {} })).toThrow(WorkflowRunCatalogError);
    expect(() => toWorkflowRunResource({ id: 'run-1' })).toThrow(WorkflowRunCatalogError);
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
    produced_files: [{
      workspace_id: 'workspace-1',
      file_id: 'file-1',
      file_name: 'report.json',
      media_type: 'application/json',
      byte_count: 512,
      sha256_hex: 'abc123',
      provenance: { path: 'report.json' },
      content_path: '/api/v1/workflow-runs/run-1/produced-files/file-1/content',
      created_at: '2026-06-21T10:05:00.000Z',
    }],
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
