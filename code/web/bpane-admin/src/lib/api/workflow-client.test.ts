import { describe, expect, it, vi } from 'vitest';
import { WorkflowClient } from './workflow-client';
import type { FetchLike } from './control-client';

const WORKFLOW = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  name: 'operator-check',
  description: 'Operator smoke workflow',
  labels: { source: 'admin-test' },
  latest_version: 'v1',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

const VERSION = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f7',
  workflow_definition_id: WORKFLOW.id,
  version: 'v1',
  executor: 'manual',
  entrypoint: 'workflows/operator/run.mjs',
  input_schema: { type: 'object' },
  output_schema: null,
  default_session: null,
  allowed_credential_binding_ids: [],
  allowed_extension_ids: [],
  allowed_file_workspace_ids: [],
  created_at: '2026-05-04T19:02:00Z',
};

const RUN = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f8',
  workflow_definition_id: WORKFLOW.id,
  workflow_definition_version_id: VERSION.id,
  workflow_version: 'v1',
  source_system: null,
  source_reference: null,
  client_request_id: 'admin-run-1',
  state: 'queued',
  session_id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f9',
  automation_task_id: '019df4d2-f4f7-7b00-9e0c-79683b1c8300',
  input: { task: 'inspect' },
  output: null,
  error: null,
  artifact_refs: [],
  produced_files: [],
  intervention: { pending_request: null },
  runtime: null,
  labels: {},
  started_at: null,
  completed_at: null,
  events_path: `/api/v1/workflow-runs/019df4d2-f4f7-7b00-9e0c-79683b1c82f8/events`,
  logs_path: `/api/v1/workflow-runs/019df4d2-f4f7-7b00-9e0c-79683b1c82f8/logs`,
  created_at: '2026-05-04T19:03:00Z',
  updated_at: '2026-05-04T19:04:00Z',
};

describe('WorkflowClient', () => {
  it('lists workflow definitions with owner bearer auth', async () => {
    const fetchImpl = jsonFetch({ workflows: [WORKFLOW] });
    const client = newClient(fetchImpl);

    const response = await client.listDefinitions();

    expect(response.workflows[0]?.latest_version).toBe('v1');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/workflows'),
      expect.objectContaining({
        method: 'GET',
        headers: expect.objectContaining({ authorization: 'Bearer owner-token' }),
      }),
    );
  });

  it('encodes workflow and version identifiers', async () => {
    const fetchImpl = jsonFetch(VERSION);
    const client = newClient(fetchImpl);

    const response = await client.getDefinitionVersion('workflow/with/slash', 'v1/candidate');

    expect(response.executor).toBe('manual');
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/workflows/workflow%2Fwith%2Fslash/versions/v1%2Fcandidate'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('creates workflow runs for an existing selected session', async () => {
    const fetchImpl = jsonFetch(RUN);
    const client = newClient(fetchImpl);

    const response = await client.createRun({
      workflow_id: WORKFLOW.id,
      version: 'v1',
      session: { existing_session_id: RUN.session_id },
      input: { task: 'inspect' },
      client_request_id: 'admin-run-1',
    });

    expect(response.session_id).toBe(RUN.session_id);
    expect(fetchImpl).toHaveBeenCalledWith(
      new URL('http://localhost:8932/api/v1/workflow-runs'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          workflow_id: WORKFLOW.id,
          version: 'v1',
          session: { existing_session_id: RUN.session_id },
          input: { task: 'inspect' },
          client_request_id: 'admin-run-1',
        }),
      }),
    );
  });

  it('controls workflow run cancellation and resume', async () => {
    const fetchImpl = jsonFetchSequence([{ ...RUN, state: 'cancelled' }, { ...RUN, state: 'running' }]);
    const client = newClient(fetchImpl);

    const cancelled = await client.cancelRun('run/with/slash');
    const resumed = await client.resumeRun(RUN.id, { comment: 'release hold' });

    expect(cancelled.state).toBe('cancelled');
    expect(resumed.state).toBe('running');
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL('http://localhost:8932/api/v1/workflow-runs/run%2Fwith%2Fslash/cancel'),
      expect.objectContaining({ method: 'POST' }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL(`http://localhost:8932/api/v1/workflow-runs/${RUN.id}/resume`),
      expect.objectContaining({ body: JSON.stringify({ comment: 'release hold' }) }),
    );
  });

  it('loads workflow run logs, events, and produced files', async () => {
    const producedFile = {
      workspace_id: 'workspace-1',
      file_id: 'file-1',
      file_name: 'report.json',
      media_type: 'application/json',
      byte_count: 20,
      sha256_hex: 'abc123',
      provenance: { source: 'workflow' },
      content_path: `/api/v1/workflow-runs/${RUN.id}/produced-files/file-1/content`,
      created_at: '2026-05-04T19:05:00Z',
    };
    const fetchImpl = jsonFetchSequence([
      { events: [{ id: 'event-1', run_id: RUN.id, source: 'run', automation_task_id: null,
        event_type: 'workflow_run.started', message: 'started', data: null, created_at: RUN.created_at }] },
      { logs: [{ id: 'log-1', run_id: RUN.id, source: 'run', automation_task_id: null,
        stream: 'stdout', message: 'hello', created_at: RUN.created_at }] },
      { files: [producedFile] },
    ]);
    const client = newClient(fetchImpl);

    const events = await client.listRunEvents(RUN.id);
    const logs = await client.listRunLogs(RUN.id);
    const files = await client.listProducedFiles(RUN.id);

    expect(events.events[0]?.event_type).toBe('workflow_run.started');
    expect(logs.logs[0]?.message).toBe('hello');
    expect(files.files[0]?.file_name).toBe('report.json');
  });
});

function newClient(fetchImpl: ReturnType<typeof vi.fn<FetchLike>>): WorkflowClient {
  return new WorkflowClient({
    baseUrl: 'http://localhost:8932',
    accessTokenProvider: () => 'owner-token',
    fetchImpl,
  });
}

function jsonFetch(payload: unknown): ReturnType<typeof vi.fn<FetchLike>> {
  return vi.fn<FetchLike>(async () => jsonResponse(payload));
}

function jsonFetchSequence(payloads: readonly unknown[]): ReturnType<typeof vi.fn<FetchLike>> {
  const queue = [...payloads];
  return vi.fn<FetchLike>(async () => jsonResponse(queue.shift()));
}

function jsonResponse(payload: unknown): Response {
  return new Response(JSON.stringify(payload), {
    status: 200,
    headers: { 'content-type': 'application/json' },
  });
}
