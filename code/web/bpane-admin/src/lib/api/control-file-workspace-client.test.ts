import { describe, expect, it, vi } from 'vitest';
import { ControlClient, type FetchLike } from './control-client';

const WORKSPACE = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f6',
  name: 'Admin inputs',
  description: 'Reusable smoke inputs',
  labels: { suite: 'admin' },
  files_path: '/api/v1/file-workspaces/019df4d2-f4f7-7b00-9e0c-79683b1c82f6/files',
  created_at: '2026-05-04T19:00:00Z',
  updated_at: '2026-05-04T19:01:00Z',
};

const WORKSPACE_FILE = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f7',
  workspace_id: WORKSPACE.id,
  name: 'customer-sample.csv',
  media_type: 'text/csv',
  byte_count: 22,
  sha256_hex: '64ec88ca00b268e5ba1a35678a1b5316d212f4f366b2477232534a8aeca37f3c',
  provenance: { source: 'admin-upload' },
  content_path: `/api/v1/file-workspaces/${WORKSPACE.id}/files/019df4d2-f4f7-7b00-9e0c-79683b1c82f7/content`,
  created_at: '2026-05-04T19:02:00Z',
  updated_at: '2026-05-04T19:03:00Z',
};

const BINDING = {
  id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f8',
  session_id: '019df4d2-f4f7-7b00-9e0c-79683b1c82f9',
  workspace_id: WORKSPACE.id,
  file_id: WORKSPACE_FILE.id,
  file_name: WORKSPACE_FILE.name,
  media_type: WORKSPACE_FILE.media_type,
  byte_count: WORKSPACE_FILE.byte_count,
  sha256_hex: WORKSPACE_FILE.sha256_hex,
  provenance: WORKSPACE_FILE.provenance,
  mount_path: 'uploads/customer-sample.csv',
  mode: 'read_only',
  state: 'pending',
  error: null,
  labels: {},
  content_path: '/api/v1/sessions/019df4d2-f4f7-7b00-9e0c-79683b1c82f9/file-bindings/019df4d2-f4f7-7b00-9e0c-79683b1c82f8/content',
  created_at: '2026-05-04T19:04:00Z',
  updated_at: '2026-05-04T19:05:00Z',
};

describe('ControlClient file workspaces and session file bindings', () => {
  it('creates and lists file workspaces with owner bearer auth', async () => {
    const fetchImpl = jsonFetchSequence([WORKSPACE, { workspaces: [WORKSPACE] }]);
    const client = newClient(fetchImpl);

    const created = await client.createFileWorkspace({
      name: 'Admin inputs',
      description: 'Reusable smoke inputs',
      labels: { suite: 'admin' },
    });
    const listed = await client.listFileWorkspaces();

    expect(created.id).toBe(WORKSPACE.id);
    expect(listed.workspaces[0]?.name).toBe('Admin inputs');
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL('http://localhost:8932/api/v1/file-workspaces'),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          name: 'Admin inputs',
          description: 'Reusable smoke inputs',
          labels: { suite: 'admin' },
        }),
        headers: expect.objectContaining({ authorization: 'Bearer owner-token' }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      2,
      new URL('http://localhost:8932/api/v1/file-workspaces'),
      expect.objectContaining({ method: 'GET' }),
    );
  });

  it('uploads, downloads, and deletes workspace files with raw content headers', async () => {
    const body = new Blob(['customer,total\n1,2\n'], { type: 'text/csv' });
    const fetchImpl = vi.fn<FetchLike>(async (_input, init) => {
      if (init?.method === 'GET' && String(_input).endsWith('/content')) {
        return new Response('customer,total\n1,2\n', { status: 200 });
      }
      return jsonResponse(WORKSPACE_FILE);
    });
    const client = newClient(fetchImpl);

    const uploaded = await client.uploadFileWorkspaceFile(WORKSPACE.id, {
      fileName: 'customer-sample.csv',
      mediaType: 'text/csv',
      provenance: { source: 'admin-upload' },
      content: body,
    });
    const downloaded = await client.downloadFileWorkspaceFileContent(uploaded);
    await client.deleteFileWorkspaceFile(WORKSPACE.id, uploaded.id);

    expect(await downloaded.text()).toBe('customer,total\n1,2\n');
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL(`http://localhost:8932/api/v1/file-workspaces/${WORKSPACE.id}/files`),
      expect.objectContaining({
        method: 'POST',
        body,
        headers: expect.objectContaining({
          'content-type': 'text/csv',
          'x-bpane-file-name': 'customer-sample.csv',
          'x-bpane-file-provenance': JSON.stringify({ source: 'admin-upload' }),
        }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      new URL(`http://localhost:8932/api/v1/file-workspaces/${WORKSPACE.id}/files/${WORKSPACE_FILE.id}`),
      expect.objectContaining({ method: 'DELETE' }),
    );
  });

  it('creates, lists, and removes session file bindings', async () => {
    const fetchImpl = jsonFetchSequence([BINDING, { bindings: [BINDING] }, BINDING]);
    const client = newClient(fetchImpl);

    const created = await client.createSessionFileBinding(BINDING.session_id, {
      workspace_id: WORKSPACE.id,
      file_id: WORKSPACE_FILE.id,
      mount_path: 'uploads/customer-sample.csv',
    });
    const listed = await client.listSessionFileBindings(BINDING.session_id);
    await client.removeSessionFileBinding(BINDING.session_id, created.id);

    expect(created.mount_path).toBe('uploads/customer-sample.csv');
    expect(listed.bindings[0]?.file_name).toBe('customer-sample.csv');
    expect(fetchImpl).toHaveBeenNthCalledWith(
      1,
      new URL(`http://localhost:8932/api/v1/sessions/${BINDING.session_id}/file-bindings`),
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({
          workspace_id: WORKSPACE.id,
          file_id: WORKSPACE_FILE.id,
          mount_path: 'uploads/customer-sample.csv',
          labels: {},
        }),
      }),
    );
    expect(fetchImpl).toHaveBeenNthCalledWith(
      3,
      new URL(`http://localhost:8932/api/v1/sessions/${BINDING.session_id}/file-bindings/${BINDING.id}`),
      expect.objectContaining({ method: 'DELETE' }),
    );
  });
});

function newClient(fetchImpl: ReturnType<typeof vi.fn<FetchLike>>): ControlClient {
  return new ControlClient({
    baseUrl: 'http://localhost:8932',
    accessTokenProvider: () => 'owner-token',
    fetchImpl,
  });
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
